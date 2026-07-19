use cuemol_core::{CueMolError, Result, Vector4};
use cuemol_qsys::{
    InOutHandler, ObjReader, ObjWriter, QdfReader, QdfWriter, QDF_TYPE_FLOAT32, QDF_TYPE_INT32,
    QDF_TYPE_INT8, QDF_TYPE_UTF8STR,
};

use crate::{ElePotMap, MolSurfObj, SurfaceVertex};

/// Re-exported so `cuemol_surface::qdf::qdf_file_type` keeps working; the QDF
/// container itself now lives in `cuemol_qsys::qdf_stream`.
pub use cuemol_qsys::qdf_file_type;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QdfSurfReader {
    reader: ObjReader,
}

impl QdfSurfReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("qdfsurf", "CueMol molsurf file (*.qdf)", "*.qdf"),
        }
    }

    pub fn reader(&self) -> &ObjReader {
        &self.reader
    }

    pub fn reader_mut(&mut self) -> &mut ObjReader {
        &mut self.reader
    }

    pub fn handler(&self) -> &InOutHandler {
        self.reader.handler()
    }

    pub fn create_default_obj(&self) -> MolSurfObj {
        MolSurfObj::new()
    }

    pub fn read_bytes(&self, input: &[u8]) -> Result<MolSurfObj> {
        read_qdf_surface_bytes(input)
    }
}

impl Default for QdfSurfReader {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QdfSurfWriter {
    writer: ObjWriter,
}

impl QdfSurfWriter {
    pub fn new() -> Self {
        Self {
            writer: ObjWriter::new("qdfsurf", "CueMol molsurf file (*.qdf)", "*.qdf"),
        }
    }

    pub fn writer(&self) -> &ObjWriter {
        &self.writer
    }

    pub fn writer_mut(&mut self) -> &mut ObjWriter {
        &mut self.writer
    }

    pub fn handler(&self) -> &InOutHandler {
        self.writer.handler()
    }

    pub fn can_handle(&self, _object: &MolSurfObj) -> bool {
        true
    }

    pub fn write_bytes(&self, object: &MolSurfObj) -> Vec<u8> {
        write_qdf_surface_bytes(object)
    }
}

impl Default for QdfSurfWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QdfPotReader {
    reader: ObjReader,
}

impl QdfPotReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("qdfpot", "CueMol elepot file (*.qdf)", "*.qdf"),
        }
    }

    pub fn reader(&self) -> &ObjReader {
        &self.reader
    }

    pub fn reader_mut(&mut self) -> &mut ObjReader {
        &mut self.reader
    }

    pub fn handler(&self) -> &InOutHandler {
        self.reader.handler()
    }

    pub fn create_default_obj(&self) -> ElePotMap {
        ElePotMap::new()
    }

    pub fn read_bytes(&self, input: &[u8]) -> Result<ElePotMap> {
        read_qdf_potential_bytes(input)
    }
}

impl Default for QdfPotReader {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QdfPotWriter {
    writer: ObjWriter,
}

impl QdfPotWriter {
    pub fn new() -> Self {
        Self {
            writer: ObjWriter::new("qdfpot", "CueMol elepot file (*.qdf)", "*.qdf"),
        }
    }

    pub fn writer(&self) -> &ObjWriter {
        &self.writer
    }

    pub fn writer_mut(&mut self) -> &mut ObjWriter {
        &mut self.writer
    }

    pub fn handler(&self) -> &InOutHandler {
        self.writer.handler()
    }

    pub fn can_handle(&self, _object: &ElePotMap) -> bool {
        true
    }

    pub fn write_bytes(&self, object: &ElePotMap) -> Vec<u8> {
        write_qdf_potential_bytes(object)
    }
}

impl Default for QdfPotWriter {
    fn default() -> Self {
        Self::new()
    }
}

pub fn read_qdf_surface_bytes(input: &[u8]) -> Result<MolSurfObj> {
    let mut cursor = QdfReader::from_bytes(input)?;
    cursor.expect_file_type("SRF1")?;

    let vertex_count = cursor.read_data_def("vert")?;
    cursor.expect_record_def(&[
        ("x", QDF_TYPE_FLOAT32),
        ("y", QDF_TYPE_FLOAT32),
        ("z", QDF_TYPE_FLOAT32),
        ("nx", QDF_TYPE_FLOAT32),
        ("ny", QDF_TYPE_FLOAT32),
        ("nz", QDF_TYPE_FLOAT32),
        ("id", QDF_TYPE_UTF8STR),
    ])?;

    let mut object = MolSurfObj::new();
    let mesh = object.mesh_mut();
    for _ in 0..vertex_count {
        let position = Vector4::new3(
            cursor.read_f32()? as f64,
            cursor.read_f32()? as f64,
            cursor.read_f32()? as f64,
        );
        let normal = Vector4::new3(
            cursor.read_f32()? as f64,
            cursor.read_f32()? as f64,
            cursor.read_f32()? as f64,
        );
        let _id = cursor.read_string()?;
        mesh.push_vertex(SurfaceVertex::new(position, normal));
    }

    let face_count = cursor.read_data_def("face")?;
    cursor.expect_record_def(&[
        ("id1", QDF_TYPE_INT32),
        ("id2", QDF_TYPE_INT32),
        ("id3", QDF_TYPE_INT32),
    ])?;
    for _ in 0..face_count {
        mesh.push_triangle([
            cursor.read_nonnegative_i32()? as u32,
            cursor.read_nonnegative_i32()? as u32,
            cursor.read_nonnegative_i32()? as u32,
        ]);
    }

    Ok(object)
}

pub fn write_qdf_surface_bytes(object: &MolSurfObj) -> Vec<u8> {
    let mut writer = QdfWriter::new("SRF1");
    let mesh = object.mesh();

    writer.data_def("vert", mesh.vertex_count());
    writer.record_def(&[
        ("x", QDF_TYPE_FLOAT32),
        ("y", QDF_TYPE_FLOAT32),
        ("z", QDF_TYPE_FLOAT32),
        ("nx", QDF_TYPE_FLOAT32),
        ("ny", QDF_TYPE_FLOAT32),
        ("nz", QDF_TYPE_FLOAT32),
        ("id", QDF_TYPE_UTF8STR),
    ]);
    for vertex in mesh.vertices() {
        writer.f32(vertex.position.x as f32);
        writer.f32(vertex.position.y as f32);
        writer.f32(vertex.position.z as f32);
        writer.f32(vertex.normal.x as f32);
        writer.f32(vertex.normal.y as f32);
        writer.f32(vertex.normal.z as f32);
        writer.string("");
    }

    writer.data_def("face", mesh.triangle_count());
    writer.record_def(&[
        ("id1", QDF_TYPE_INT32),
        ("id2", QDF_TYPE_INT32),
        ("id3", QDF_TYPE_INT32),
    ]);
    for triangle in mesh.triangles() {
        writer.i32(triangle[0] as i32);
        writer.i32(triangle[1] as i32);
        writer.i32(triangle[2] as i32);
    }

    writer.finish()
}

pub fn read_qdf_potential_bytes(input: &[u8]) -> Result<ElePotMap> {
    let mut cursor = QdfReader::from_bytes(input)?;
    cursor.expect_file_type("POT1")?;

    let header_count = cursor.read_data_def("hdr")?;
    if header_count != 1 {
        return Err(CueMolError::parse("QDF potential header length must be 1"));
    }
    cursor.expect_record_def(&[
        ("nx", QDF_TYPE_INT32),
        ("ny", QDF_TYPE_INT32),
        ("nz", QDF_TYPE_INT32),
        ("gx", QDF_TYPE_FLOAT32),
        ("gy", QDF_TYPE_FLOAT32),
        ("gz", QDF_TYPE_FLOAT32),
        ("ox", QDF_TYPE_FLOAT32),
        ("oy", QDF_TYPE_FLOAT32),
        ("oz", QDF_TYPE_FLOAT32),
        ("prec", QDF_TYPE_INT8),
    ])?;

    let counts = [
        cursor.read_nonnegative_i32()? as usize,
        cursor.read_nonnegative_i32()? as usize,
        cursor.read_nonnegative_i32()? as usize,
    ];
    let grid_size = [
        cursor.read_f32()? as f64,
        cursor.read_f32()? as f64,
        cursor.read_f32()? as f64,
    ];
    let origin = Vector4::new3(
        cursor.read_f32()? as f64,
        cursor.read_f32()? as f64,
        cursor.read_f32()? as f64,
    );
    let precision = cursor.read_i8()?;
    if precision != 1 {
        return Err(CueMolError::parse("unsupported QDF potential precision"));
    }

    let expected = counts[0]
        .checked_mul(counts[1])
        .and_then(|n| n.checked_mul(counts[2]))
        .ok_or_else(|| CueMolError::parse("QDF potential grid is too large"))?;
    let data_count = cursor.read_data_def("data")?;
    if data_count != expected {
        return Err(CueMolError::parse("QDF potential data count mismatch"));
    }
    cursor.expect_record_def(&[("val", QDF_TYPE_FLOAT32)])?;

    let mut values = vec![0.0; expected];
    for ix in 0..counts[0] {
        for iy in 0..counts[1] {
            for iz in 0..counts[2] {
                let index = ix + counts[0] * (iy + counts[1] * iz);
                values[index] = cursor.read_f32()? as f64;
            }
        }
    }

    let mut map = ElePotMap::new();
    map.set_map_float_array(&values, counts, grid_size, origin);
    Ok(map)
}

pub fn write_qdf_potential_bytes(map: &ElePotMap) -> Vec<u8> {
    let mut writer = QdfWriter::new("POT1");

    writer.data_def("hdr", 1);
    writer.record_def(&[
        ("nx", QDF_TYPE_INT32),
        ("ny", QDF_TYPE_INT32),
        ("nz", QDF_TYPE_INT32),
        ("gx", QDF_TYPE_FLOAT32),
        ("gy", QDF_TYPE_FLOAT32),
        ("gz", QDF_TYPE_FLOAT32),
        ("ox", QDF_TYPE_FLOAT32),
        ("oy", QDF_TYPE_FLOAT32),
        ("oz", QDF_TYPE_FLOAT32),
        ("prec", QDF_TYPE_INT8),
    ]);
    writer.i32(map.col_no() as i32);
    writer.i32(map.row_no() as i32);
    writer.i32(map.sec_no() as i32);
    writer.f32(map.col_grid_size() as f32);
    writer.f32(map.row_grid_size() as f32);
    writer.f32(map.sec_grid_size() as f32);
    let origin = map.origin();
    writer.f32(origin.x as f32);
    writer.f32(origin.y as f32);
    writer.f32(origin.z as f32);
    writer.i8(1);

    writer.data_def(
        "data",
        map.col_no()
            .saturating_mul(map.row_no())
            .saturating_mul(map.sec_no()),
    );
    writer.record_def(&[("val", QDF_TYPE_FLOAT32)]);
    for ix in 0..map.col_no() {
        for iy in 0..map.row_no() {
            for iz in 0..map.sec_no() {
                writer.f32(map.at_float(ix, iy, iz) as f32);
            }
        }
    }

    writer.finish()
}
