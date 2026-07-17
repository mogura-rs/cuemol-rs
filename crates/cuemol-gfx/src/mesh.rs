use std::{fmt, sync::Arc};

use cuemol_core::{CueMolError, Result, Vector4};

use crate::{AbstractColor, ColorPtr};

#[derive(Clone)]
pub struct Mesh {
    vertex_count: usize,
    face_count: usize,
    vertices: Vec<f32>,
    normals: Vec<f32>,
    colors: Vec<Option<ColorPtr>>,
    faces: Vec<i32>,
    current_normal: Vector4,
    current_color: Option<ColorPtr>,
}

impl Default for Mesh {
    fn default() -> Self {
        Self {
            vertex_count: 0,
            face_count: 0,
            vertices: Vec::new(),
            normals: Vec::new(),
            colors: Vec::new(),
            faces: Vec::new(),
            current_normal: Vector4::ZERO,
            current_color: None,
        }
    }
}

impl fmt::Debug for Mesh {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Mesh")
            .field("vertex_count", &self.vertex_count)
            .field("face_count", &self.face_count)
            .field("vertices", &self.vertices)
            .field("normals", &self.normals)
            .field("faces", &self.faces)
            .field("current_normal", &self.current_normal)
            .finish_non_exhaustive()
    }
}

impl Mesh {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init(&mut self, vertex_count: usize, face_count: usize) {
        self.vertex_count = vertex_count;
        self.face_count = face_count;
        self.vertices = vec![0.0; vertex_count * 3];
        self.normals = vec![0.0; vertex_count * 3];
        self.colors = vec![None; vertex_count];
        self.faces = vec![0; face_count * 3];
    }

    pub fn reduce(&mut self, vertex_count: usize, face_count: usize) -> bool {
        if self.vertex_count < vertex_count || self.face_count < face_count {
            return false;
        }
        self.vertex_count = vertex_count;
        self.face_count = face_count;
        true
    }

    pub fn vertex_size(&self) -> usize {
        self.vertex_count
    }

    pub fn get_vert_size(&self) -> usize {
        self.vertex_size()
    }

    pub fn face_size(&self) -> usize {
        self.face_count
    }

    pub fn get_face_size(&self) -> usize {
        self.face_size()
    }

    pub fn normal(&mut self, value: Vector4) {
        self.current_normal = value;
    }

    pub fn color(&mut self, value: ColorPtr) {
        self.current_color = Some(value);
    }

    pub fn set_color<C>(&mut self, value: C)
    where
        C: AbstractColor + Send + Sync + 'static,
    {
        self.color(Arc::new(value));
    }

    pub fn set_vertex(&mut self, index: usize, value: Vector4) -> Result<()> {
        self.set_vertex_components(index, value.x as f32, value.y as f32, value.z as f32)
    }

    pub fn set_vertex_components(&mut self, index: usize, x: f32, y: f32, z: f32) -> Result<()> {
        self.check_vertex_index(index)?;
        let offset = index * 3;
        self.vertices[offset..offset + 3].copy_from_slice(&[x, y, z]);
        self.normals[offset..offset + 3].copy_from_slice(&[
            self.current_normal.x as f32,
            self.current_normal.y as f32,
            self.current_normal.z as f32,
        ]);
        self.colors[index] = self.current_color.clone();
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_vertex_with_normal(
        &mut self,
        index: usize,
        x: f32,
        y: f32,
        z: f32,
        nx: f32,
        ny: f32,
        nz: f32,
    ) -> Result<()> {
        self.check_vertex_index(index)?;
        let offset = index * 3;
        self.vertices[offset..offset + 3].copy_from_slice(&[x, y, z]);
        self.normals[offset..offset + 3].copy_from_slice(&[nx, ny, nz]);
        self.colors[index] = self.current_color.clone();
        Ok(())
    }

    pub fn vertex(&self, index: usize) -> Result<Vector4> {
        self.check_vertex_index(index)?;
        let offset = index * 3;
        Ok(Vector4::new3(
            f64::from(self.vertices[offset]),
            f64::from(self.vertices[offset + 1]),
            f64::from(self.vertices[offset + 2]),
        ))
    }

    pub fn get_vertex(&self, index: usize) -> Result<Vector4> {
        self.vertex(index)
    }

    pub fn normal_at(&self, index: usize) -> Result<Vector4> {
        self.check_vertex_index(index)?;
        let offset = index * 3;
        Ok(Vector4::new3(
            f64::from(self.normals[offset]),
            f64::from(self.normals[offset + 1]),
            f64::from(self.normals[offset + 2]),
        ))
    }

    pub fn get_normal(&self, index: usize) -> Result<Vector4> {
        self.normal_at(index)
    }

    pub fn color_at(&self, index: usize) -> Option<ColorPtr> {
        if index >= self.vertex_count {
            return None;
        }
        self.colors[index].clone()
    }

    pub fn get_color(&self, index: usize) -> Option<ColorPtr> {
        self.color_at(index)
    }

    pub fn set_face(
        &mut self,
        face_index: usize,
        vertex1: i32,
        vertex2: i32,
        vertex3: i32,
    ) -> Result<()> {
        if face_index >= self.face_count {
            return Err(CueMolError::invalid_argument(
                "mesh face index out of range",
            ));
        }
        let offset = face_index * 3;
        self.faces[offset..offset + 3].copy_from_slice(&[vertex1, vertex2, vertex3]);
        Ok(())
    }

    pub fn float_vertices(&self) -> &[f32] {
        &self.vertices[..self.vertex_count * 3]
    }

    pub fn get_float_vertices(&self) -> &[f32] {
        self.float_vertices()
    }

    pub fn float_normals(&self) -> &[f32] {
        &self.normals[..self.vertex_count * 3]
    }

    pub fn get_float_normals(&self) -> &[f32] {
        self.float_normals()
    }

    pub fn faces(&self) -> &[i32] {
        &self.faces[..self.face_count * 3]
    }

    pub fn get_faces(&self) -> &[i32] {
        self.faces()
    }

    pub fn convert_rgba_byte_colors(
        &self,
        output: &mut [u8],
        default_alpha: u8,
        scene_id: Option<u64>,
    ) -> bool {
        if output.len() < self.vertex_count * 4 {
            return false;
        }
        for index in 0..self.vertex_count {
            let Some(color) = &self.colors[index] else {
                return false;
            };
            let code = scene_id.map_or_else(|| color.code(), |id| color.get_dev_code(id));
            let offset = index * 4;
            output[offset] = ((code >> 16) & 0xff) as u8;
            output[offset + 1] = ((code >> 8) & 0xff) as u8;
            output[offset + 2] = (code & 0xff) as u8;
            output[offset + 3] = ((code >> 24) & 0xff) as u8;
            if default_alpha != u8::MAX {
                output[offset + 3] =
                    ((u16::from(output[offset + 3]) * u16::from(default_alpha)) / 255) as u8;
            }
        }
        true
    }

    pub fn conv_rgba_byte_cols(
        &self,
        output: &mut [u8],
        default_alpha: u8,
        scene_id: Option<u64>,
    ) -> bool {
        self.convert_rgba_byte_colors(output, default_alpha, scene_id)
    }

    fn check_vertex_index(&self, index: usize) -> Result<()> {
        if index >= self.vertex_count {
            Err(CueMolError::invalid_argument(
                "mesh vertex index out of range",
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SolidColor;

    #[test]
    fn mesh_records_current_attributes_faces_and_active_sizes() {
        let mut mesh = Mesh::new();
        mesh.init(3, 1);
        mesh.normal(Vector4::new3(0.0, 0.0, 1.0));
        mesh.set_color(SolidColor::from_rgba_u8(10, 20, 30, 128));
        mesh.set_vertex(0, Vector4::new3(1.0, 2.0, 3.0)).unwrap();
        mesh.set_vertex_with_normal(1, 4.0, 5.0, 6.0, 0.0, 1.0, 0.0)
            .unwrap();
        mesh.set_vertex(2, Vector4::new3(7.0, 8.0, 9.0)).unwrap();
        mesh.set_face(0, 0, 1, 2).unwrap();
        assert_eq!(mesh.vertex(0).unwrap(), Vector4::new3(1.0, 2.0, 3.0));
        assert_eq!(mesh.normal_at(1).unwrap(), Vector4::new3(0.0, 1.0, 0.0));
        assert_eq!(mesh.faces(), &[0, 1, 2]);
        assert!(mesh.reduce(2, 1));
        assert_eq!(mesh.float_vertices(), &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert!(!mesh.reduce(3, 1));
    }

    #[test]
    fn mesh_converts_colors_and_multiplies_default_alpha() {
        let mut mesh = Mesh::new();
        mesh.init(1, 0);
        mesh.set_color(SolidColor::from_rgba_u8(10, 20, 30, 128));
        mesh.set_vertex(0, Vector4::ZERO).unwrap();
        let mut bytes = [0; 4];
        assert!(mesh.convert_rgba_byte_colors(&mut bytes, 128, None));
        assert_eq!(bytes, [10, 20, 30, 64]);
        assert!(!mesh.convert_rgba_byte_colors(&mut bytes[..3], 255, None));
    }
}
