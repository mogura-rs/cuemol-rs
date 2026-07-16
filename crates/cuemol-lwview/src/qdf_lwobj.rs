//! QDF lightweight-object reader/writer. This is the binary `LWO1`/`LWO2`/
//! `LWO3` layout used by C++ `lwview::QdfLWObjReader` and `QdfLWObjWriter`.

use cuemol_core::{CueMolError, Result, Vector4};
use cuemol_gfx::PixelBuffer;
use cuemol_qsys::{
    InOutHandler, ObjReader, ObjWriter, QdfReader, QdfWriter, QDF_TYPE_FLOAT32, QDF_TYPE_INT32,
    QDF_TYPE_INT8, QDF_TYPE_RGBA, QDF_TYPE_UTF8STR, QDF_TYPE_VEC3,
};
use std::path::Path;

use crate::{
    LWDrawData, LWDrawElement, LWObject, LWPixelData, LWVertexColor, LWVertexNormalColor,
    LW_DRAW_PIXEL, LW_DRAW_V, LW_DRAW_VC, LW_DRAW_VNCI,
};

const TYPE_RECORD: &[(&str, i8)] = &[
    ("id", QDF_TYPE_INT32),
    ("t", QDF_TYPE_INT8),
    ("m", QDF_TYPE_INT8),
    ("w", QDF_TYPE_FLOAT32),
    ("c", QDF_TYPE_RGBA),
];
const VATR_RECORD: &[(&str, i8)] = &[("v", QDF_TYPE_VEC3)];
const VCAT_RECORD: &[(&str, i8)] = &[("v", QDF_TYPE_VEC3), ("c", QDF_TYPE_RGBA)];
const VNCA_RECORD: &[(&str, i8)] = &[
    ("v", QDF_TYPE_VEC3),
    ("n", QDF_TYPE_VEC3),
    ("c", QDF_TYPE_RGBA),
];
const VNCI_RECORD: &[(&str, i8)] = &[("i", QDF_TYPE_INT32)];
const PIXI_RECORD: &[(&str, i8)] = &[
    ("v", QDF_TYPE_VEC3),
    ("c", QDF_TYPE_RGBA),
    ("w", QDF_TYPE_INT32),
    ("h", QDF_TYPE_INT32),
    ("d", QDF_TYPE_INT8),
];
const PIXD_RECORD: &[(&str, i8)] = &[("d", QDF_TYPE_INT8)];
const HITM_RECORD: &[(&str, i8)] = &[("v", QDF_TYPE_VEC3), ("s", QDF_TYPE_UTF8STR)];
const HITT_RECORD: &[(&str, i8)] = &[("id", QDF_TYPE_INT32), ("t", QDF_TYPE_INT8)];
const HITP_RECORD: &[(&str, i8)] = &[("i", QDF_TYPE_INT32)];

#[derive(Debug, Clone, Copy)]
struct DrawHeader {
    data_id: i32,
    type_id: i8,
    draw_mode: i8,
    line_width: f32,
    default_color: [u8; 4],
}

/// Serialize an `LWObject` to the C++-compatible QDF `LWO3` stream.
pub fn write_qdf_lwobj_bytes(obj: &LWObject) -> Vec<u8> {
    let mut writer = QdfWriter::new("LWO3");

    writer.data_def("type", obj.draw_elements().len());
    writer.record_def(TYPE_RECORD);
    for element in obj.draw_elements() {
        writer.i32(element.data_id());
        writer.i8(element.type_id());
        writer.i8(element.draw_mode());
        writer.f32(element.line_width());
        writer.rgba(element.default_color());
    }

    for element in obj.draw_elements() {
        write_draw_element(&mut writer, element);
    }

    writer.data_def("hitm", obj.hit_data_len());
    writer.record_def(HITM_RECORD);
    for index in 0..obj.hit_data_len() as i32 {
        let hit = obj.hit_data(index).expect("hit index in range");
        writer.vec3(vector_to_f32(hit.position()));
        writer.string(hit.message());
    }

    writer.data_def("hitt", obj.hit_index_sets().len());
    writer.record_def(HITT_RECORD);
    for set in obj.hit_index_sets() {
        writer.i32(set.data_id());
        writer.i8(set.hit_type());
    }
    for set in obj.hit_index_sets() {
        writer.data_def("hitp", set.indices().len());
        writer.record_def(HITP_RECORD);
        for &index in set.indices() {
            writer.i32(index);
        }
    }

    writer.finish()
}

fn write_draw_element(writer: &mut QdfWriter, element: &LWDrawElement) {
    match element.data() {
        LWDrawData::Vertices(vertices) => {
            writer.data_def("vatr", vertices.len());
            writer.record_def(VATR_RECORD);
            for &position in vertices {
                writer.vec3(position);
            }
        }
        LWDrawData::VertexColors(vertices) => {
            writer.data_def("vcat", vertices.len());
            writer.record_def(VCAT_RECORD);
            for vertex in vertices {
                writer.vec3(vertex.position);
                writer.rgba(vertex.color);
            }
        }
        LWDrawData::VertexNormalColorIndices { vertices, indices } => {
            writer.data_def("vnca", vertices.len());
            writer.record_def(VNCA_RECORD);
            for vertex in vertices {
                writer.vec3(vertex.position);
                writer.vec3(vertex.normal);
                writer.rgba(vertex.color);
            }
            writer.data_def("vnci", indices.len());
            writer.record_def(VNCI_RECORD);
            for &index in indices {
                writer.i32(index);
            }
        }
        LWDrawData::Pixel(pixel) => {
            writer.data_def("pixi", 1);
            writer.record_def(PIXI_RECORD);
            writer.vec3(pixel.position());
            writer.rgba(pixel.color());
            writer.i32(pixel.pixels().width() as i32);
            writer.i32(pixel.pixels().height() as i32);
            writer.i8(pixel.pixels().depth() as i8);

            writer.data_def("pixd", pixel.pixels().size());
            writer.record_def(PIXD_RECORD);
            for &byte in pixel.pixels().data() {
                writer.i8(byte as i8);
            }
        }
    }
}

/// Parse a C++ QDF `LWO1`, `LWO2`, or `LWO3` stream.
pub fn read_qdf_lwobj_bytes(input: &[u8]) -> Result<LWObject> {
    let mut reader = QdfReader::from_bytes(input)?;
    let version = match reader.file_type() {
        "LWO1" => 1,
        "LWO2" => 2,
        "LWO3" => 3,
        other => {
            return Err(CueMolError::parse(format!(
                "QDF lightweight object type mismatch: {other}"
            )))
        }
    };

    let count = reader.read_data_def("type")?;
    reader.expect_record_def(TYPE_RECORD)?;
    let mut headers = Vec::with_capacity(count);
    for _ in 0..count {
        headers.push(DrawHeader {
            data_id: reader.read_i32()?,
            type_id: reader.read_i8()?,
            draw_mode: reader.read_i8()?,
            line_width: reader.read_f32()?,
            default_color: reader.read_rgba()?,
        });
    }

    let mut object = LWObject::new();
    for header in headers {
        let data = match header.type_id {
            LW_DRAW_V => read_vertices(&mut reader)?,
            LW_DRAW_VC => read_vertex_colors(&mut reader)?,
            LW_DRAW_VNCI => read_vertex_normal_color_indices(&mut reader)?,
            LW_DRAW_PIXEL => read_pixel(&mut reader)?,
            _ => {
                reader.skip_all_records()?;
                continue;
            }
        };
        let draw_mode = if header.type_id == LW_DRAW_PIXEL {
            1
        } else {
            header.draw_mode
        };
        object.add_draw_element(LWDrawElement::new(
            header.data_id,
            draw_mode,
            header.line_width,
            header.default_color,
            data,
        ));
    }

    if version >= 2 {
        read_hit_data(&mut reader, &mut object)?;
    }
    Ok(object)
}

fn read_vertices(reader: &mut QdfReader<'_>) -> Result<LWDrawData> {
    let count = reader.read_data_def("vatr")?;
    reader.expect_record_def(VATR_RECORD)?;
    let vertices = (0..count)
        .map(|_| reader.read_vec3())
        .collect::<Result<Vec<_>>>()?;
    Ok(LWDrawData::Vertices(vertices))
}

fn read_vertex_colors(reader: &mut QdfReader<'_>) -> Result<LWDrawData> {
    let count = reader.read_data_def("vcat")?;
    reader.expect_record_def(VCAT_RECORD)?;
    let vertices = (0..count)
        .map(|_| {
            Ok(LWVertexColor {
                position: reader.read_vec3()?,
                color: reader.read_rgba()?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(LWDrawData::VertexColors(vertices))
}

fn read_vertex_normal_color_indices(reader: &mut QdfReader<'_>) -> Result<LWDrawData> {
    let count = reader.read_data_def("vnca")?;
    reader.expect_record_def(VNCA_RECORD)?;
    let vertices = (0..count)
        .map(|_| {
            Ok(LWVertexNormalColor {
                position: reader.read_vec3()?,
                normal: reader.read_vec3()?,
                color: reader.read_rgba()?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let index_count = reader.read_data_def("vnci")?;
    reader.expect_record_def(VNCI_RECORD)?;
    let indices = (0..index_count)
        .map(|_| reader.read_i32())
        .collect::<Result<Vec<_>>>()?;
    Ok(LWDrawData::VertexNormalColorIndices { vertices, indices })
}

fn read_pixel(reader: &mut QdfReader<'_>) -> Result<LWDrawData> {
    if reader.read_data_def("pixi")? != 1 {
        return Err(CueMolError::parse("QDF LWO pixel-info count must be 1"));
    }
    reader.expect_record_def(PIXI_RECORD)?;
    let position = reader.read_vec3()?;
    let color = reader.read_rgba()?;
    let width = reader.read_nonnegative_i32()? as usize;
    let height = reader.read_nonnegative_i32()? as usize;
    let depth = reader.read_nonnegative_i32_from_i8()?;
    if !matches!(depth, 8 | 32) {
        return Err(CueMolError::parse("QDF LWO pixel depth must be 8 or 32"));
    }

    let size = reader.read_data_def("pixd")?;
    reader.expect_record_def(PIXD_RECORD)?;
    let mut pixels = PixelBuffer::new();
    pixels.set_width(width);
    pixels.set_height(height);
    pixels.set_depth(depth);
    pixels.resize(size);
    for byte in pixels.data_mut() {
        *byte = reader.read_i8()? as u8;
    }
    Ok(LWDrawData::Pixel(LWPixelData::new(position, color, pixels)))
}

fn read_hit_data(reader: &mut QdfReader<'_>, object: &mut LWObject) -> Result<()> {
    let hit_count = reader.read_data_def("hitm")?;
    reader.expect_record_def(HITM_RECORD)?;
    for index in 0..hit_count {
        let position = reader.read_vec3()?;
        let message = reader.read_string()?;
        object.add_point_hit(index as i32, f32_to_vector(position), message);
    }

    let set_count = reader.read_data_def("hitt")?;
    reader.expect_record_def(HITT_RECORD)?;
    let sets = (0..set_count)
        .map(|_| Ok((reader.read_i32()?, reader.read_i8()?)))
        .collect::<Result<Vec<_>>>()?;
    for (data_id, hit_type) in sets {
        let index_count = reader.read_data_def("hitp")?;
        reader.expect_record_def(HITP_RECORD)?;
        let indices = (0..index_count)
            .map(|_| reader.read_i32())
            .collect::<Result<Vec<_>>>()?;
        object.set_hit_index_set(data_id, hit_type, indices);
    }
    Ok(())
}

fn vector_to_f32(value: Vector4) -> [f32; 3] {
    [value.x as f32, value.y as f32, value.z as f32]
}

fn f32_to_vector(value: [f32; 3]) -> Vector4 {
    Vector4::new3(value[0] as f64, value[1] as f64, value[2] as f64)
}

trait QdfReaderExt {
    fn read_nonnegative_i32_from_i8(&mut self) -> Result<usize>;
}

impl QdfReaderExt for QdfReader<'_> {
    fn read_nonnegative_i32_from_i8(&mut self) -> Result<usize> {
        let value = self.read_i8()?;
        if value < 0 {
            Err(CueMolError::parse(
                "QDF negative 8-bit integer is invalid here",
            ))
        } else {
            Ok(value as usize)
        }
    }
}

/// `ObjReader` facade for QDF lightweight objects (handler name `qdflwobj`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QdfLWObjReader {
    reader: ObjReader,
}

impl QdfLWObjReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("qdflwobj", "CueMol light-weight obj file (*.qdf)", "*.qdf"),
        }
    }

    pub fn handler(&self) -> &InOutHandler {
        self.reader.handler()
    }

    pub fn create_default_obj(&self) -> LWObject {
        LWObject::new()
    }

    pub fn read_bytes(&self, input: &[u8]) -> Result<LWObject> {
        read_qdf_lwobj_bytes(input)
    }

    pub fn read_file(&self, path: impl AsRef<Path>) -> Result<LWObject> {
        read_qdf_lwobj_bytes(&std::fs::read(path)?)
    }
}

impl Default for QdfLWObjReader {
    fn default() -> Self {
        Self::new()
    }
}

/// `ObjWriter` facade for QDF lightweight objects (handler name `qdflwobj`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QdfLWObjWriter {
    writer: ObjWriter,
}

impl QdfLWObjWriter {
    pub fn new() -> Self {
        Self {
            writer: ObjWriter::new("qdflwobj", "CueMol light-weight obj file (*.qdf)", "*.qdf"),
        }
    }

    pub fn handler(&self) -> &InOutHandler {
        self.writer.handler()
    }

    pub fn can_handle(&self, _object: &LWObject) -> bool {
        true
    }

    pub fn write_bytes(&self, object: &LWObject) -> Vec<u8> {
        write_qdf_lwobj_bytes(object)
    }

    pub fn write_file(&self, object: &LWObject, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path, write_qdf_lwobj_bytes(object))?;
        Ok(())
    }
}

impl Default for QdfLWObjWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_qsys::qdf_file_type;

    fn sample() -> LWObject {
        let mut object = LWObject::new();
        object.add_draw_element(LWDrawElement::new(
            10,
            7,
            1.25,
            [10, 20, 30, 255],
            LWDrawData::Vertices(vec![[1.0, 2.0, 3.0], [-4.0, 5.0, 6.0]]),
        ));
        object.add_draw_element(LWDrawElement::new(
            10,
            4,
            2.5,
            [1, 2, 3, 4],
            LWDrawData::VertexColors(vec![LWVertexColor {
                position: [7.0, 8.0, 9.0],
                color: [100, 110, 120, 130],
            }]),
        ));
        object.add_draw_element(LWDrawElement::new(
            20,
            7,
            1.0,
            [255, 255, 255, 255],
            LWDrawData::VertexNormalColorIndices {
                vertices: vec![LWVertexNormalColor {
                    position: [1.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    color: [255, 0, 0, 255],
                }],
                indices: vec![0, 0, 0],
            },
        ));
        let mut pixels = PixelBuffer::new();
        pixels.set_width(2);
        pixels.set_height(1);
        pixels.set_depth(32);
        pixels.resize(8);
        pixels.data_mut().copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        object.add_draw_element(LWDrawElement::new(
            30,
            1,
            3.0,
            [4, 3, 2, 1],
            LWDrawData::Pixel(LWPixelData::new(
                [0.25, 0.5, 0.75],
                [11, 22, 33, 44],
                pixels,
            )),
        ));
        object.add_point_hit(42, Vector4::new3(1.0, 2.0, 3.0), "atom:CA");
        object.add_point_hit(7, Vector4::new3(-4.0, 5.0, 6.0), "bond");
        object.set_hit_index_set(10, 0, vec![0, 1]);
        object
    }

    #[test]
    fn qdf_lwobj_roundtrip_preserves_cpp_lwo3_records() {
        let source = sample();
        let bytes = write_qdf_lwobj_bytes(&source);
        assert_eq!(qdf_file_type(&bytes).as_deref(), Some("LWO3"));

        let result = read_qdf_lwobj_bytes(&bytes).unwrap();
        assert_eq!(result.draw_elements(), source.draw_elements());
        assert_eq!(result.hit_data_len(), 2);
        assert_eq!(result.hit_data(0).unwrap().message(), "atom:CA");
        assert_eq!(result.hit_data(1).unwrap().message(), "bond");
        assert_eq!(result.hit_index_sets(), source.hit_index_sets());
        assert_eq!(result.draw_elements()[3].draw_mode(), 1);
    }

    #[test]
    fn qdf_lwobj_emits_reference_block_order_and_schemas() {
        let bytes = write_qdf_lwobj_bytes(&sample());
        let mut reader = QdfReader::from_bytes(&bytes).unwrap();
        assert_eq!(reader.read_data_def("type").unwrap(), 4);
        reader.expect_record_def(TYPE_RECORD).unwrap();
        for _ in 0..4 {
            reader.read_i32().unwrap();
            reader.read_i8().unwrap();
            reader.read_i8().unwrap();
            reader.read_f32().unwrap();
            reader.read_rgba().unwrap();
        }
        assert_eq!(reader.read_data_def("vatr").unwrap(), 2);
        reader.expect_record_def(VATR_RECORD).unwrap();
        reader.read_vec3().unwrap();
        reader.read_vec3().unwrap();
        assert_eq!(reader.read_data_def("vcat").unwrap(), 1);
        reader.expect_record_def(VCAT_RECORD).unwrap();
    }

    #[test]
    fn read_rejects_non_lwo_stream() {
        let bytes = QdfWriter::new("MOL2").finish();
        assert!(read_qdf_lwobj_bytes(&bytes).is_err());
    }

    #[test]
    fn handler_metadata_matches_reference() {
        let reader = QdfLWObjReader::new();
        assert_eq!(reader.handler().name(), "qdflwobj");
        assert_eq!(reader.handler().file_ext(), "*.qdf");
        let writer = QdfLWObjWriter::new();
        assert!(writer.can_handle(&LWObject::new()));
    }
}
