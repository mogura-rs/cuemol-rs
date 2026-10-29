use super::*;
use cuemol_core::{CueMolError, Result};

/// Parsed grid storage and statistics, without scene IDs or renderer state.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DensityMapData {
    counts: [usize; 3],
    start: [isize; 3],
    intervals: [usize; 3],
    origin: Vector4,
    delta: [Vector4; 3],
    cell: [f64; 6],
    space_group: i32,
    statistics: [f64; 6],
    bytes: Arc<Vec<u8>>,
    histogram: Vec<u64>,
    continuous_values: Option<Arc<[f32]>>,
    map_type: u8,
    detected_map_type: u8,
}

impl DensityMap {
    pub fn transfer_data(&self) -> DensityMapData {
        DensityMapData {
            counts: self.counts,
            start: self.start,
            intervals: self.intervals,
            origin: self.origin,
            delta: self.delta,
            cell: [
                self.cell.a,
                self.cell.b,
                self.cell.c,
                self.cell.alpha,
                self.cell.beta,
                self.cell.gamma,
            ],
            space_group: self.cell.nsg,
            statistics: [
                self.min_density,
                self.max_density,
                self.mean_density,
                self.rmsd_density,
                self.level_base,
                self.level_step,
            ],
            bytes: self.bytes.clone(),
            histogram: self.histogram.to_vec(),
            continuous_values: self.continuous_values.clone(),
            map_type: self.map_type,
            detected_map_type: self.detected_map_type,
        }
    }
}

impl DensityMapData {
    pub fn into_map(self) -> Result<DensityMap> {
        let length = self
            .counts
            .iter()
            .try_fold(1usize, |length, count| length.checked_mul(*count));
        if length != Some(self.bytes.len())
            || self
                .continuous_values
                .as_ref()
                .is_some_and(|values| Some(values.len()) != length)
        {
            return Err(CueMolError::parse(
                "prepared map dimensions do not match its values",
            ));
        }
        let histogram = self
            .histogram
            .try_into()
            .map_err(|_| CueMolError::parse("prepared map histogram must have 256 bins"))?;
        let [a, b, c, alpha, beta, gamma] = self.cell;
        let [min_density, max_density, mean_density, rmsd_density, level_base, level_step] =
            self.statistics;
        Ok(DensityMap {
            counts: self.counts,
            start: self.start,
            intervals: self.intervals,
            origin: self.origin,
            delta: self.delta,
            cell: CrystalInfo::with_space_group(a, b, c, alpha, beta, gamma, self.space_group),
            min_density,
            max_density,
            mean_density,
            rmsd_density,
            level_base,
            level_step,
            bytes: self.bytes,
            histogram,
            continuous_values: self.continuous_values,
            map_type: self.map_type,
            detected_map_type: self.detected_map_type,
        })
    }
}
