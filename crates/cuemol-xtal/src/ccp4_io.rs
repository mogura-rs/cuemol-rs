use crate::{Ccp4Header, Ccp4Map, Ccp4MapReader, DensityMap, GridMap, MapStats};
use cuemol_core::{CueMolError, Result};
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;

trait MapInput: Read + Seek {}
impl<T: Read + Seek> MapInput for T {}

#[cfg(not(target_arch = "wasm32"))]
fn header(input: &mut (impl Read + ?Sized)) -> Result<Ccp4Header> {
    let mut bytes = [0; 1024];
    input.read_exact(&mut bytes)?;
    Ccp4Header::parse(&bytes)
}

fn checked_header(reader: &Ccp4MapReader, header: &Ccp4Header) -> Result<Ccp4Header> {
    let sampled = header.sampled(reader.subsample)?;
    let count = sampled.voxel_count()?;
    if reader.max_voxels > 0. && count as f64 > reader.max_voxels {
        return Err(CueMolError::parse(format!(
            "CCP4 map too large ({count} voxels > max_voxels {}); use subsample or raise max_voxels",
            reader.max_voxels
        )));
    }
    header.data_end()?;
    cuemol_molstr::map::ccp4_delta_vectors(&sampled)?;
    Ok(sampled)
}

fn compressed(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x1f, 0x8b])
        || bytes.starts_with(b"BZh")
        || bytes.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0])
}

#[cfg(not(target_arch = "wasm32"))]
fn with_file<T>(
    reader: &Ccp4MapReader,
    path: &Path,
    read: impl FnOnce(&mut dyn MapInput, Ccp4Header) -> Result<T>,
) -> Result<T> {
    let mut file = std::fs::File::open(path)?;
    let mut prefix = [0; 8];
    file.read_exact(&mut prefix)?;
    file.seek(SeekFrom::Start(0))?;
    if !compressed(&prefix) {
        let info = header(&mut file)?;
        checked_header(reader, &info)?;
        return read(&mut file, info);
    }
    let info = Ccp4Header::parse(&cuemol_core::compressed::read(path, 1024)?)?;
    checked_header(reader, &info)?;
    let mut decoded = tempfile::tempfile()?;
    cuemol_core::compressed::copy_to(path, &mut decoded, info.data_end()?)?;
    read(&mut decoded, info)
}

#[cfg(target_arch = "wasm32")]
fn with_file<T>(
    _reader: &Ccp4MapReader,
    _path: &Path,
    _read: impl FnOnce(&mut dyn MapInput, Ccp4Header) -> Result<T>,
) -> Result<T> {
    Err(CueMolError::invalid_argument(
        "browser map input requires bytes",
    ))
}

fn with_bytes<T>(
    reader: &Ccp4MapReader,
    bytes: &[u8],
    read: impl FnOnce(&mut dyn MapInput, Ccp4Header) -> Result<T>,
) -> Result<T> {
    if compressed(bytes) {
        let info = Ccp4Header::parse(&cuemol_core::compressed::decode_prefix(bytes, 1024)?)?;
        checked_header(reader, &info)?;
        let decoded = cuemol_core::compressed::decode_bytes(bytes)?;
        return with_bytes(reader, &decoded, read);
    }
    let info = Ccp4Header::parse(bytes)?;
    checked_header(reader, &info)?;
    read(&mut Cursor::new(bytes), info)
}

fn validate_extent(input: &mut dyn MapInput, info: &Ccp4Header) -> Result<()> {
    if input.seek(SeekFrom::End(0))? < info.data_end()? {
        return Err(CueMolError::parse("CCP4 data is shorter than header count"));
    }
    Ok(())
}

fn transform(reader: &Ccp4MapReader, info: &Ccp4Header, mut value: f64) -> f64 {
    if reader.truncate_min {
        value = value.max(reader.min * info.stats.rms);
    }
    if reader.truncate_max {
        value = value.min(reader.max * info.stats.rms);
    }
    if reader.normalize && info.stats.rms != 0. {
        value = (value - info.stats.mean) / info.stats.rms;
    }
    value
}

#[derive(Default)]
struct Stats {
    min: f64,
    max: f64,
    sum: f64,
    square: f64,
    count: u64,
}
impl Stats {
    fn add(&mut self, value: f64) {
        if self.count == 0 {
            self.min = value;
            self.max = value;
        }
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.sum += value;
        self.square += value * value;
        self.count += 1;
    }
    fn finish(&self) -> MapStats {
        let mean = self.sum / self.count as f64;
        MapStats {
            min: self.min,
            max: self.max,
            mean,
            rms: (self.square / self.count as f64 - mean * mean)
                .max(0.)
                .sqrt(),
        }
    }
}

fn visit(
    reader: &Ccp4MapReader,
    input: &mut dyn MapInput,
    info: &Ccp4Header,
    mut sink: impl FnMut(usize, f64),
) -> Result<MapStats> {
    let mut stats = Stats::default();
    info.visit_values(input, reader.subsample, |index, value| {
        let value = f64::from(transform(reader, info, f64::from(value)) as f32);
        if !value.is_finite() {
            return Err(CueMolError::parse("invalid transformed CCP4 density"));
        }
        stats.add(value);
        sink(index, value);
        Ok(())
    })?;
    Ok(stats.finish())
}

fn quant_range(min: f64, max: f64) -> (f64, f64) {
    let step = (max - min) / 256.;
    (min, if step > 0. { step } else { 1. })
}

fn read_density(
    reader: &Ccp4MapReader,
    input: &mut dyn MapInput,
    info: Ccp4Header,
) -> Result<DensityMap> {
    let sampled = checked_header(reader, &info)?;
    validate_extent(input, &info)?;
    let hs = info.stats;
    let use_header = reader.use_header_stats
        && hs.min.is_finite()
        && hs.max.is_finite()
        && hs.mean.is_finite()
        && hs.min.abs() <= 1e30
        && hs.max.abs() <= 1e30
        && hs.min < hs.max
        && hs.mean >= hs.min
        && hs.mean <= hs.max;
    let (mut base, mut step) = if use_header {
        quant_range(
            transform(reader, &info, hs.min),
            transform(reader, &info, hs.max),
        )
    } else {
        let stats = visit(reader, input, &info, |_, _| {})?;
        quant_range(stats.min, stats.max)
    };
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(sampled.voxel_count()?)
        .map_err(|e| CueMolError::parse(e.to_string()))?;
    bytes.resize(sampled.voxel_count()?, 0);
    let stats = visit(reader, input, &info, |index, value| {
        bytes[index] = ((value - base) / step).clamp(0., 255.) as u8;
    })?;
    if use_header && (stats.min < base - 0.5 * step || stats.max > base + 256.5 * step) {
        (base, step) = quant_range(stats.min, stats.max);
        visit(reader, input, &info, |index, value| {
            bytes[index] = ((value - base) / step).clamp(0., 255.) as u8;
        })?;
    }
    let mut map = DensityMap::new();
    map.set_quantized_data(bytes, sampled.counts, stats, base, step);
    map.set_map_params(sampled.start.map(|v| v as isize), sampled.grid_intervals);
    let [a, b, c, alpha, beta, gamma] = info.cell;
    map.set_xtal_params(a, b, c, alpha, beta, gamma);
    map.set_space_group(info.space_group);
    map.set_origin(info.cell_origin());
    map.set_detected_map_type(info.detected_map_type());
    Ok(map)
}

fn read_grid(
    reader: &Ccp4MapReader,
    input: &mut dyn MapInput,
    info: Ccp4Header,
) -> Result<Ccp4Map> {
    let sampled = checked_header(reader, &info)?;
    validate_extent(input, &info)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(sampled.voxel_count()?)
        .map_err(|e| CueMolError::parse(e.to_string()))?;
    values.resize(sampled.voxel_count()?, 0.);
    visit(reader, input, &info, |index, value| values[index] = value)?;
    let delta = cuemol_molstr::map::ccp4_delta_vectors(&sampled)?;
    let origin = info.cell_origin()
        + delta[0] * f64::from(sampled.start[0])
        + delta[1] * f64::from(sampled.start[1])
        + delta[2] * f64::from(sampled.start[2]);
    Ok(Ccp4Map {
        grid: GridMap {
            counts: sampled.counts,
            origin,
            delta,
            values,
        },
        header: sampled,
    })
}

pub(crate) fn density_file(reader: &Ccp4MapReader, path: &Path) -> Result<DensityMap> {
    with_file(reader, path, |input, info| {
        read_density(reader, input, info)
    })
}
pub(crate) fn density_bytes(reader: &Ccp4MapReader, bytes: &[u8]) -> Result<DensityMap> {
    with_bytes(reader, bytes, |input, info| {
        read_density(reader, input, info)
    })
}
pub(crate) fn grid_file(reader: &Ccp4MapReader, path: &Path) -> Result<Ccp4Map> {
    with_file(reader, path, |input, info| read_grid(reader, input, info))
}
pub(crate) fn grid_bytes(reader: &Ccp4MapReader, bytes: &[u8]) -> Result<Ccp4Map> {
    with_bytes(reader, bytes, |input, info| read_grid(reader, input, info))
}

pub(crate) fn probe(path: &Path) -> Result<String> {
    probe_bytes(&cuemol_core::compressed::read(path, 1024)?)
}
pub(crate) fn probe_bytes(bytes: &[u8]) -> Result<String> {
    let h = Ccp4Header::parse(bytes)?;
    Ok(serde_json::json!({
        "nc": h.raw_counts[0], "nr": h.raw_counts[1], "ns": h.raw_counts[2],
        "mode": h.mode, "supported": h.bytes_per_voxel() > 0,
        "nvoxels": h.voxel_count()?, "file_bytes_per_voxel": h.bytes_per_voxel(),
        "storage_bytes": h.voxel_count()?, "start": h.raw_start, "grid": h.raw_grid,
        "cell": h.cell, "axis": h.axis_order.map(|v| v + 1),
        "ispg": h.space_group, "nsymbt": h.symmetry_bytes, "nversion": h.nversion,
        "exttyp": h.exttyp, "origin": h.origin,
        "dmin": h.stats.min, "dmax": h.stats.max, "dmean": h.stats.mean, "rms": h.stats.rms,
    })
    .to_string())
}
