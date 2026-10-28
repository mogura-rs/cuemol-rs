//! Rasterized element identities shared by viewport interaction and scripts.

use cuemol_qsys::ViewHoverHit;
use std::sync::{Arc, Mutex};

#[cfg(feature = "offscreen")]
mod gpu;
#[cfg(feature = "offscreen")]
pub use gpu::GpuPicking;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickStamp {
    pub revision: u64,
    pub matrix: [[f32; 4]; 4],
    pub size: [u32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickRecord {
    pub hit: ViewHoverHit,
    pub object_id: u64,
    pub atom_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PickImage {
    pub stamp: PickStamp,
    pub size: [u32; 2],
    pub pixels: Vec<[u32; 4]>,
    pub records: Arc<[PickRecord]>,
}

pub type PickCache = Arc<Mutex<Option<Arc<PickImage>>>>;

impl PickImage {
    pub fn hit_at(
        &self,
        point: [f64; 2],
        viewport: [f64; 2],
        precision: f64,
    ) -> Option<PickRecord> {
        if viewport.iter().any(|n| !n.is_finite() || *n <= 0.)
            || point.iter().any(|n| !n.is_finite())
            || point[0] < 0.
            || point[1] < 0.
            || point[0] >= viewport[0]
            || point[1] >= viewport[1]
        {
            return None;
        }
        let scale = [
            self.size[0] as f64 / viewport[0],
            self.size[1] as f64 / viewport[1],
        ];
        let center = [point[0] * scale[0], point[1] * scale[1]];
        let half = [
            precision.max(1.) * scale[0] * 0.5,
            precision.max(1.) * scale[1] * 0.5,
        ];
        let lo = [
            ((center[0] - half[0]).floor() as i64).max(0) as u32,
            ((center[1] - half[1]).floor() as i64).max(0) as u32,
        ];
        let hi = [
            ((center[0] + half[0]).ceil() as u32).min(self.size[0]),
            ((center[1] + half[1]).ceil() as u32).min(self.size[1]),
        ];
        let mut best: Option<(u32, f64, u32)> = None;
        for y in lo[1]..hi[1] {
            for x in lo[0]..hi[0] {
                let pixel = self.pixels[(y * self.size[0] + x) as usize];
                if pixel[0] == 0 || pixel[0] as usize > self.records.len() {
                    continue;
                }
                let distance =
                    (x as f64 + 0.5 - center[0]).powi(2) + (y as f64 + 0.5 - center[1]).powi(2);
                if best.is_none_or(|old| (pixel[1], distance) < (old.0, old.1)) {
                    best = Some((pixel[1], distance, pixel[0]));
                }
            }
        }
        best.and_then(|(_, _, id)| self.records.get(id as usize - 1).copied())
    }
}

pub fn cpu_hit(
    snapshot: &cuemol_render::RenderSceneSnapshot,
    matrix: [[f32; 4]; 4],
    point: [f64; 2],
    viewport: [f64; 2],
    precision: f64,
    cpu_only: bool,
) -> Option<PickRecord> {
    if point
        .iter()
        .chain(viewport.iter())
        .any(|value| !value.is_finite())
        || viewport.iter().any(|value| *value <= 0.)
    {
        return None;
    }
    let mut best: Option<((f64, f64), PickRecord)> = None;
    for group in snapshot
        .transparency_groups
        .iter()
        .filter_map(|section| section.picking.as_ref())
        .filter(|group| !cpu_only || !group.gpu_supported)
    {
        for atom in group.atoms.iter() {
            let p = [
                atom.position.x as f32,
                atom.position.y as f32,
                atom.position.z as f32,
                1.,
            ];
            let clip: [f32; 4] = std::array::from_fn(|i| (0..4).map(|j| matrix[j][i] * p[j]).sum());
            if clip[3] <= 1e-6 {
                continue;
            }
            let depth = (clip[2] / clip[3]) as f64;
            if !(0. ..=1.).contains(&depth) {
                continue;
            }
            let x = (1. + (clip[0] / clip[3]) as f64) * 0.5 * viewport[0];
            let y = (1. - (clip[1] / clip[3]) as f64) * 0.5 * viewport[1];
            let dx = x - point[0];
            let dy = y - point[1];
            let score = (depth, dx * dx + dy * dy);
            if dx.abs() <= precision * 0.5
                && dy.abs() <= precision * 0.5
                && best.is_none_or(|(old, _)| score < old)
            {
                best = Some((
                    score,
                    PickRecord {
                        object_id: group.object_id,
                        atom_index: atom.atom_index,
                        hit: ViewHoverHit {
                            renderer_id: cuemol_core::Uid::new(group.renderer_id),
                            atom_id: atom.atom_id,
                            symm_id: group.symm_id,
                        },
                    },
                ));
            }
        }
    }
    best.map(|(_, record)| record)
}
