//! QDF density-map reader/writer — CueMol's native `*.qdf` container for an
//! electron-density `DensityMap`. Ports C++ `xtal::QdfDenMapReader` /
//! `QdfDenMapWriter`.
//!
//! The stream (file type `"MAP1"`) carries three data blocks:
//! - `xtal` (1 record): unit cell `a,b,c,alp,bet,gam` + space-group number `sg`.
//! - `hdr`  (1 record): `mode` (1 = byte map), grid counts `nx,ny,nz`, start
//!   indices `stax,stay,staz`, cell intervals `intx,inty,intz`, and the density
//!   stats `rmin,rmax,rmea,rsig`.
//! - `bmap` (`nx*ny*nz` records): one `int8` per voxel, the quantized byte map
//!   in column-fastest order (`i + nx*(j + ny*k)`).
//!
//! The map origin / per-axis deltas are NOT stored — they are reconstructed from
//! the cell and intervals (origin at the cell origin), matching the
//! crystallographic convention C++ uses.

use cuemol_core::Result;
use cuemol_qsys::{
    InOutHandler, ObjReader, ObjWriter, QdfReader, QdfWriter, ScalarObject, QDF_TYPE_FLOAT32,
    QDF_TYPE_INT32, QDF_TYPE_INT8,
};

use crate::map_renderers::DensityMap;

const XTAL_RECORD: &[(&str, i8)] = &[
    ("a", QDF_TYPE_FLOAT32),
    ("b", QDF_TYPE_FLOAT32),
    ("c", QDF_TYPE_FLOAT32),
    ("alp", QDF_TYPE_FLOAT32),
    ("bet", QDF_TYPE_FLOAT32),
    ("gam", QDF_TYPE_FLOAT32),
    ("sg", QDF_TYPE_INT32),
];

const HDR_RECORD: &[(&str, i8)] = &[
    ("mode", QDF_TYPE_INT8),
    ("nx", QDF_TYPE_INT32),
    ("ny", QDF_TYPE_INT32),
    ("nz", QDF_TYPE_INT32),
    ("stax", QDF_TYPE_INT32),
    ("stay", QDF_TYPE_INT32),
    ("staz", QDF_TYPE_INT32),
    ("intx", QDF_TYPE_INT32),
    ("inty", QDF_TYPE_INT32),
    ("intz", QDF_TYPE_INT32),
    ("rmin", QDF_TYPE_FLOAT32),
    ("rmax", QDF_TYPE_FLOAT32),
    ("rmea", QDF_TYPE_FLOAT32),
    ("rsig", QDF_TYPE_FLOAT32),
];

const BMAP_RECORD: &[(&str, i8)] = &[("v", QDF_TYPE_INT8)];

/// Serialize a `DensityMap` to a QDF `MAP1` byte stream.
pub fn write_qdf_denmap_bytes(map: &DensityMap) -> Vec<u8> {
    let mut w = QdfWriter::new("MAP1");
    let cell = map.cell();

    // xtal block
    w.data_def("xtal", 1);
    w.record_def(XTAL_RECORD);
    w.f32(cell.a as f32);
    w.f32(cell.b as f32);
    w.f32(cell.c as f32);
    w.f32(cell.alpha as f32);
    w.f32(cell.beta as f32);
    w.f32(cell.gamma as f32);
    w.i32(cell.space_group());

    // hdr block
    let (nx, ny, nz) = (map.col_no(), map.row_no(), map.sec_no());
    let intervals = map.intervals();
    w.data_def("hdr", 1);
    w.record_def(HDR_RECORD);
    w.i8(1); // mode 1 = byte map
    w.i32(nx as i32);
    w.i32(ny as i32);
    w.i32(nz as i32);
    w.i32(map.start_col() as i32);
    w.i32(map.start_row() as i32);
    w.i32(map.start_sec() as i32);
    w.i32(intervals[0] as i32);
    w.i32(intervals[1] as i32);
    w.i32(intervals[2] as i32);
    w.f32(map.den_min() as f32);
    w.f32(map.den_max() as f32);
    w.f32(map.den_mean() as f32);
    w.f32(map.den_sigma() as f32);

    // bmap block: the quantized voxel bytes, column-fastest (matches the C++
    // k/j/i loop and DensityMap's flat layout `i + nx*(j + ny*k)`).
    let nsize = nx.saturating_mul(ny).saturating_mul(nz);
    w.data_def("bmap", nsize);
    w.record_def(BMAP_RECORD);
    for &b in map.byte_values().iter().take(nsize) {
        w.i8(b as i8);
    }

    w.finish()
}

/// Parse a QDF `MAP1` byte stream into a `DensityMap`.
pub fn read_qdf_denmap_bytes(input: &[u8]) -> Result<DensityMap> {
    use cuemol_core::CueMolError;

    let mut r = QdfReader::from_bytes(input)?;
    r.expect_file_type("MAP1")?;

    // xtal block
    if r.read_data_def("xtal")? != 1 {
        return Err(CueMolError::parse("QDF map xtal block length must be 1"));
    }
    r.expect_record_def(XTAL_RECORD)?;
    let a = r.read_f32()? as f64;
    let b = r.read_f32()? as f64;
    let c = r.read_f32()? as f64;
    let alpha = r.read_f32()? as f64;
    let beta = r.read_f32()? as f64;
    let gamma = r.read_f32()? as f64;
    let sg = r.read_i32()?;

    // hdr block
    if r.read_data_def("hdr")? != 1 {
        return Err(CueMolError::parse("QDF map hdr block length must be 1"));
    }
    r.expect_record_def(HDR_RECORD)?;
    let mode = r.read_i8()?;
    if mode != 1 {
        return Err(CueMolError::parse(
            "QDF map: only byte-map mode (1) is supported",
        ));
    }
    let nx = r.read_nonnegative_i32()? as usize;
    let ny = r.read_nonnegative_i32()? as usize;
    let nz = r.read_nonnegative_i32()? as usize;
    let start = [
        r.read_i32()? as isize,
        r.read_i32()? as isize,
        r.read_i32()? as isize,
    ];
    let intervals = [
        r.read_nonnegative_i32()? as usize,
        r.read_nonnegative_i32()? as usize,
        r.read_nonnegative_i32()? as usize,
    ];
    let rmin = r.read_f32()? as f64;
    let rmax = r.read_f32()? as f64;
    let rmea = r.read_f32()? as f64;
    let rsig = r.read_f32()? as f64;

    // bmap block
    let nsize = nx
        .checked_mul(ny)
        .and_then(|n| n.checked_mul(nz))
        .ok_or_else(|| CueMolError::parse("QDF map grid is too large"))?;
    if r.read_data_def("bmap")? != nsize {
        return Err(CueMolError::parse("QDF map byte count mismatch"));
    }
    r.expect_record_def(BMAP_RECORD)?;
    let mut bytes = Vec::with_capacity(nsize);
    for _ in 0..nsize {
        bytes.push(r.read_i8()? as u8);
    }

    // Reconstruct: stats+bytes, then start/intervals, then cell-derived deltas,
    // then the space group (set_xtal_params resets it to 1).
    let mut map = DensityMap::new();
    map.set_map_byte_array(&bytes, [nx, ny, nz], rmin, rmax, rmea, rsig);
    map.set_map_params(start, intervals);
    map.set_xtal_params(a, b, c, alpha, beta, gamma);
    map.set_space_group(sg);
    Ok(map)
}

/// `ObjReader` facade for QDF density maps (handler name `qdfmap`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QdfDenMapReader {
    reader: ObjReader,
}

impl QdfDenMapReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("qdfmap", "CueMol density map file (*.qdf)", "*.qdf"),
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

    pub fn create_default_obj(&self) -> DensityMap {
        DensityMap::new()
    }

    pub fn read_bytes(&self, input: &[u8]) -> Result<DensityMap> {
        read_qdf_denmap_bytes(input)
    }
}

impl Default for QdfDenMapReader {
    fn default() -> Self {
        Self::new()
    }
}

/// `ObjWriter` facade for QDF density maps (handler name `qdfmap`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QdfDenMapWriter {
    writer: ObjWriter,
}

impl QdfDenMapWriter {
    pub fn new() -> Self {
        Self {
            writer: ObjWriter::new("qdfmap", "CueMol density map file (*.qdf)", "*.qdf"),
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

    pub fn can_handle(&self, _object: &DensityMap) -> bool {
        true
    }

    pub fn write_bytes(&self, object: &DensityMap) -> Vec<u8> {
        write_qdf_denmap_bytes(object)
    }
}

impl Default for QdfDenMapWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_core::Vector4;
    use cuemol_qsys::qdf_file_type;

    fn sample_map() -> DensityMap {
        // A 4x5x6 byte map with a deterministic gradient, on a non-trivial cell,
        // start, and intervals — covers every field the QDF format stores.
        let counts = [4usize, 5, 6];
        let nsize = counts[0] * counts[1] * counts[2];
        let bytes: Vec<u8> = (0..nsize).map(|i| (i * 2 % 256) as u8).collect();
        let mut map = DensityMap::new();
        // Stats chosen to be exactly f32-representable for an exact round-trip.
        map.set_map_byte_array(&bytes, counts, -1.5, 2.5, 0.5, 0.75);
        map.set_map_params([2, -1, 3], [8, 10, 12]);
        map.set_xtal_params(40.0, 50.0, 60.0, 90.0, 90.0, 90.0);
        map.set_space_group(19);
        map
    }

    #[test]
    fn qdf_denmap_roundtrip_preserves_every_stored_field() {
        let src = sample_map();
        let bytes = write_qdf_denmap_bytes(&src);
        assert_eq!(qdf_file_type(&bytes).as_deref(), Some("MAP1"));

        let dst = read_qdf_denmap_bytes(&bytes).expect("read back QDF map");

        // Cell + space group.
        let (sc, dc) = (src.cell(), dst.cell());
        assert!((sc.a - dc.a).abs() < 1.0e-4);
        assert!((sc.b - dc.b).abs() < 1.0e-4);
        assert!((sc.c - dc.c).abs() < 1.0e-4);
        assert!((sc.alpha - dc.alpha).abs() < 1.0e-4);
        assert!((sc.beta - dc.beta).abs() < 1.0e-4);
        assert!((sc.gamma - dc.gamma).abs() < 1.0e-4);
        assert_eq!(dc.space_group(), 19);

        // Grid geometry.
        assert_eq!(
            [dst.col_no(), dst.row_no(), dst.sec_no()],
            [src.col_no(), src.row_no(), src.sec_no()]
        );
        assert_eq!(
            [dst.start_col(), dst.start_row(), dst.start_sec()],
            [2, -1, 3]
        );
        assert_eq!(dst.intervals(), [8, 10, 12]);

        // Density statistics.
        assert!((dst.den_min() - src.den_min()).abs() < 1.0e-4);
        assert!((dst.den_max() - src.den_max()).abs() < 1.0e-4);
        assert!((dst.den_mean() - src.den_mean()).abs() < 1.0e-4);
        assert!((dst.den_sigma() - src.den_sigma()).abs() < 1.0e-4);

        // The byte map itself (exact).
        assert_eq!(dst.byte_values(), src.byte_values());

        // Grid geometry the Increment-9 isosurface depends on: QDF stores no
        // delta/origin, so they are reconstructed from cell+intervals. With
        // intervals [8,10,12] != counts, the spacing is non-trivial
        // (col_grid_size = a/intx = 40/8 = 5.0), so pin it and a transformed node
        // to guard against a map that loads at the wrong scale.
        assert!((dst.col_grid_size() - src.col_grid_size()).abs() < 1.0e-4);
        assert!((dst.row_grid_size() - src.row_grid_size()).abs() < 1.0e-4);
        assert!((dst.sec_grid_size() - src.sec_grid_size()).abs() < 1.0e-4);
        assert!((dst.col_grid_size() - 5.0).abs() < 1.0e-4);
        let sp = src.conv_to_orth(Vector4::new3(1.0, 1.0, 1.0));
        let dp = dst.conv_to_orth(Vector4::new3(1.0, 1.0, 1.0));
        assert!((sp.x - dp.x).abs() < 1.0e-4);
        assert!((sp.y - dp.y).abs() < 1.0e-4);
        assert!((sp.z - dp.z).abs() < 1.0e-4);
    }

    #[test]
    fn read_rejects_non_map_qdf_stream() {
        // A POT1-typed stream is a valid QDF container but the wrong object type.
        let bytes = QdfWriter::new("POT1").finish();
        assert!(read_qdf_denmap_bytes(&bytes).is_err());
    }

    #[test]
    fn reader_writer_handler_metadata_matches_reference() {
        let reader = QdfDenMapReader::new();
        assert_eq!(reader.handler().name(), "qdfmap");
        assert_eq!(
            reader.handler().type_descr(),
            "CueMol density map file (*.qdf)"
        );
        assert_eq!(reader.handler().file_ext(), "*.qdf");

        let writer = QdfDenMapWriter::new();
        assert_eq!(writer.handler().name(), "qdfmap");
        assert!(writer.can_handle(&DensityMap::new()));
    }
}
