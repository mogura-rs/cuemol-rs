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
//! The optional map-kind and origin fields match current CueMol2 files.
//! Older MAP1 headers remain readable. MAP2 splits samples into section chunks.
//! Optional `qbase`/`qstep` preserve quantization independently of measured statistics.

use std::path::Path;

use cuemol_core::Result;
use cuemol_qsys::{
    InOutHandler, ObjReader, ObjWriter, QdfReader, QdfWriter, ScalarObject, QDF_TYPE_FLOAT32,
    QDF_TYPE_FLOAT64, QDF_TYPE_INT32, QDF_TYPE_INT8,
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

const MAP_KIND_RECORD: &[(&str, i8)] = &[
    ("mtype", QDF_TYPE_INT8),
    ("orgx", QDF_TYPE_FLOAT32),
    ("orgy", QDF_TYPE_FLOAT32),
    ("orgz", QDF_TYPE_FLOAT32),
    ("qbase", QDF_TYPE_FLOAT64),
    ("qstep", QDF_TYPE_FLOAT64),
];

const BMAP_RECORD: &[(&str, i8)] = &[("v", QDF_TYPE_INT8)];

/// Serialize a `DensityMap` to a QDF `MAP1` byte stream.
pub fn write_qdf_denmap_bytes(map: &DensityMap) -> Vec<u8> {
    write_map_chunks(map, i32::MAX as usize, true)
}

fn write_map_chunks(map: &DensityMap, limit: usize, kind_fields: bool) -> Vec<u8> {
    let (nx, ny, nz) = (map.col_no(), map.row_no(), map.sec_no());
    let slice = nx.saturating_mul(ny);
    assert!(
        slice <= limit,
        "one density-map section exceeds the QDF record limit"
    );
    let sections = (limit / slice.max(1)).max(1).min(nz.max(1));
    let chunks = nz.div_ceil(sections).max(1);
    let split = chunks > 1;
    let mut w = QdfWriter::new(if split { "MAP2" } else { "MAP1" });
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
    let mut fields = HDR_RECORD.to_vec();
    if kind_fields {
        fields.extend_from_slice(MAP_KIND_RECORD);
    }
    if split {
        fields.extend([("nchk", QDF_TYPE_INT32), ("csec", QDF_TYPE_INT32)]);
    }
    w.record_def(&fields);
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
    if kind_fields {
        w.i8(map.detected_map_type() as i8);
        let origin = map.origin();
        w.f32(origin.x as f32);
        w.f32(origin.y as f32);
        w.f32(origin.z as f32);
        w.f64(map.level_base());
        w.f64(map.level_step());
    }
    if split {
        w.i32(chunks as i32);
        w.i32(sections as i32);
    }

    for chunk in 0..chunks {
        let start = chunk * sections * slice;
        let end = ((chunk + 1) * sections).min(nz) * slice;
        w.data_def("bmap", end - start);
        w.record_def(BMAP_RECORD);
        for &value in &map.byte_values()[start..end] {
            w.i8(value as i8);
        }
    }

    w.finish()
}

/// Parse a QDF `MAP1` byte stream into a `DensityMap`.
pub fn read_qdf_denmap_bytes(input: &[u8]) -> Result<DensityMap> {
    use cuemol_core::CueMolError;

    let mut r = QdfReader::from_bytes(input)?;
    let split = r.file_type() == "MAP2";
    if !split {
        r.expect_file_type("MAP1")?;
    }

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
    let fields = r.read_record_def()?;
    if fields.len() < HDR_RECORD.len()
        || fields
            .iter()
            .zip(HDR_RECORD)
            .any(|(field, (name, kind))| field.name != *name || field.type_tag != *kind)
    {
        return Err(CueMolError::parse("invalid QDF map header schema"));
    }
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
    let mut map_type = 1;
    let mut origin = cuemol_core::Vector4::ZERO;
    let mut chunks = 1;
    let mut sections = nz;
    let mut base = rmin;
    let mut step = (rmax - rmin) / 256.;
    for field in &fields[HDR_RECORD.len()..] {
        match (field.name.as_str(), field.type_tag) {
            ("mtype", QDF_TYPE_INT8) => map_type = r.read_i8()? as u8,
            ("orgx", QDF_TYPE_FLOAT32) => origin.x = r.read_f32()? as f64,
            ("orgy", QDF_TYPE_FLOAT32) => origin.y = r.read_f32()? as f64,
            ("orgz", QDF_TYPE_FLOAT32) => origin.z = r.read_f32()? as f64,
            ("qbase", QDF_TYPE_FLOAT64) => base = r.read_f64()?,
            ("qstep", QDF_TYPE_FLOAT64) => step = r.read_f64()?,
            ("nchk", QDF_TYPE_INT32) => chunks = r.read_nonnegative_i32()? as usize,
            ("csec", QDF_TYPE_INT32) => sections = r.read_nonnegative_i32()? as usize,
            _ => r.skip_field(field)?,
        }
    }
    if split && (chunks == 0 || sections == 0 || chunks != nz.div_ceil(sections)) {
        return Err(CueMolError::parse("inconsistent MAP2 section chunks"));
    }

    // bmap block
    let nsize = nx
        .checked_mul(ny)
        .and_then(|n| n.checked_mul(nz))
        .ok_or_else(|| CueMolError::parse("QDF map grid is too large"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(nsize)
        .map_err(|e| CueMolError::parse(format!("cannot allocate QDF map: {e}")))?;
    for chunk in 0..chunks {
        let count = if split {
            nx * ny * (nz - chunk * sections).min(sections)
        } else {
            nsize
        };
        if r.read_data_def("bmap")? != count {
            return Err(CueMolError::parse("QDF map byte count mismatch"));
        }
        r.expect_record_def(BMAP_RECORD)?;
        for _ in 0..count {
            bytes.push(r.read_i8()? as u8);
        }
    }

    // Reconstruct: stats+bytes, then start/intervals, then cell-derived deltas,
    // then the space group (set_xtal_params resets it to 1).
    let mut map = DensityMap::new();
    if !base.is_finite() || !step.is_finite() || step < 0. {
        return Err(CueMolError::parse("invalid QDF map quantization"));
    }
    map.set_quantized_data(
        bytes,
        [nx, ny, nz],
        cuemol_molstr::MapStats {
            min: rmin,
            max: rmax,
            mean: rmea,
            rms: rsig,
        },
        base,
        step,
    );
    map.set_map_params(start, intervals);
    map.set_xtal_params(a, b, c, alpha, beta, gamma);
    map.set_space_group(sg);
    map.set_detected_map_type(map_type);
    map.set_origin(origin);
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

    pub fn read_file(&self, path: impl AsRef<Path>) -> Result<DensityMap> {
        self.read_bytes(&std::fs::read(path)?)
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

    pub fn write_file(&self, object: &DensityMap, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path, self.write_bytes(object))?;
        Ok(())
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

        // Grid geometry the isosurface depends on: QDF stores no
        // deltas, so they are reconstructed from cell+intervals. With
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
    fn em_origin_and_chunked_maps_roundtrip_without_periodic_wrapping() {
        let mut map = sample_map();
        map.set_space_group(0);
        map.set_origin(Vector4::new3(12., -4., 2.5));
        for limit in [40, usize::MAX] {
            let bytes = write_map_chunks(&map, limit, true);
            let restored = read_qdf_denmap_bytes(&bytes).unwrap();
            assert!(restored.is_em());
            assert!(!restored.is_periodic_grid());
            assert_eq!(restored.origin(), map.origin());
            assert_eq!(restored.byte_values(), map.byte_values());
            assert_eq!(
                restored.conv_to_orth(Vector4::new3(1., 2., 3.)),
                map.conv_to_orth(Vector4::new3(1., 2., 3.))
            );
        }
        let legacy =
            read_qdf_denmap_bytes(&write_map_chunks(&sample_map(), usize::MAX, false)).unwrap();
        assert_eq!(legacy.origin(), Vector4::ZERO);
        assert!(!legacy.is_em());
    }

    #[test]
    fn subsampling_preserves_cell_and_saved_grid_positions() {
        let mut map = sample_map();
        map.set_map_params([1, -2, 3], [9, 11, 13]);
        map.set_xtal_params(45., 55., 65., 90., 90., 90.);
        map.set_origin(Vector4::new3(4., 5., 6.));
        let first = map.conv_to_orth(Vector4::ZERO);
        let cell = map.cell();
        map.subsample(2).unwrap();
        assert_eq!(map.cell(), cell);
        assert!((map.conv_to_orth(Vector4::ZERO) - first).length3d() < 1e-10);
        let restored = read_qdf_denmap_bytes(&write_qdf_denmap_bytes(&map)).unwrap();
        for position in [Vector4::ZERO, Vector4::new3(1., 2., 1.)] {
            assert!(
                (restored.conv_to_orth(position) - map.conv_to_orth(position)).length3d() < 1e-6
            );
        }
    }

    #[test]
    fn em_percent_levels_use_density_quantiles() {
        let mut map = DensityMap::new();
        map.set_map_byte_array(
            &(0u8..=255).collect::<Vec<_>>(),
            [256, 1, 1],
            0.,
            256.,
            127.5,
            73.9,
        );
        map.set_map_params([0; 3], [256, 1, 1]);
        map.set_xtal_params(256., 1., 1., 90., 90., 90.);
        map.set_space_group(0);
        let mut renderer = crate::MapRenderer::new();
        renderer.attach_client_scalar(&map);
        renderer.set_siglevel(25.);
        assert_eq!(map.level_at_top_fraction(0.25), 192.);
        assert_eq!(renderer.level(), 192.);
        assert_eq!(renderer.level_from(&map), 192.);
        renderer.set_level(128.);
        assert_eq!(renderer.siglevel(), 50.);
        map.set_map_type("xtal").unwrap();
        assert!(map.is_periodic_grid());
        assert!((renderer.level_from(&map) - 50. * 73.9).abs() < 1e-6);
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
