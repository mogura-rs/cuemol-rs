use super::meshing::{mesh_sphpat, MeshState, Tag};
use super::sas::{DataCir, DataI};
use super::sas_patches::{DataLoop, DataPat, DataSeg};
use super::vec3::Vec3;
use super::Geom;

#[inline]
fn rset3<const N: usize>(row: &mut [f64; N], c0: usize, v: Vec3) {
    row[c0] = v.x;
    row[c0 + 1] = v.y;
    row[c0 + 2] = v.z;
}

/// Writes atom `i`'s direction-corrected segments into `segment0`/`rj` and returns its circles.
#[allow(clippy::too_many_arguments)]
fn mod_seg_loop_cir(
    i: usize,
    geom: &Geom,
    di: &DataI,
    dc: &DataCir,
    ds: &DataSeg,
    segment0: &mut [[f64; 12]],
    rj: &mut [f64],
) -> Vec<[f64; 9]> {
    let r = &geom.r;
    let ii = i as i32;
    for &sa in &ds.satom[i] {
        let sa = sa as usize;
        let seg = &ds.segment[sa];
        let ncra = &ds.ncrasegment[sa];
        let direct_s = seg[4] * (2 * (seg[0] == ii) as i32 - 1);
        let direct_n = (2 * (seg[0] == ii) as i32 - 1) as f64;
        let n = Vec3::new(ncra[0], ncra[1], ncra[2]);
        let a = Vec3::new(ncra[3], ncra[4], ncra[5]);
        let start = if direct_s == -1 { seg[3] } else { seg[2] };
        let mut sv = [0.0; 12];
        rset3(&mut sv, 1, a);
        rset3(&mut sv, 4, direct_n * n);
        sv[7] = ncra[6];
        rset3(&mut sv, 8, di.i[start as usize]);
        sv[11] = ncra[7];
        segment0[sa] = sv;
        rj[sa] = if seg[0] == ii {
            r[seg[1] as usize]
        } else {
            r[seg[0] as usize]
        };
    }
    let circleindex_i = &dc.circleindex[i];
    let mut circle0 = vec![[0.0; 9]; circleindex_i.len() + 1];
    for (k, &cidx) in circleindex_i.iter().enumerate() {
        let crow = &dc.circle[cidx as usize];
        let c_i = crow[1] as usize;
        let c_j = crow[2] as usize;
        let c = Vec3::new(crow[3], crow[4], crow[5]);
        let cn = Vec3::new(crow[6], crow[7], crow[8]);
        let c0 = &mut circle0[k + 1];
        rset3(c0, 1, c);
        c0[7] = crow[9];
        if c_i == i {
            rset3(c0, 4, cn);
            c0[8] = r[c_j];
        } else {
            rset3(c0, 4, -1.0 * cn);
            c0[8] = r[c_i];
        }
    }
    circle0
}

/// Meshes every convex (contracted SAS) spherical patch into `state`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn data_sessphpat_convex(
    state: &mut MeshState,
    geom: &Geom,
    di: &DataI,
    dc: &DataCir,
    ds: &DataSeg,
    dl: &DataLoop,
    dp: &DataPat,
    rp: f64,
    d: f64,
) -> Result<(), String> {
    let c = &geom.centers;
    let r = &geom.r;
    let mut segment0 = vec![[0.0; 12]; ds.nsegment + 1];
    let mut rj = vec![0.0; ds.nsegment + 1];
    for i in 1..=geom.m {
        let nsatom_i = ds.satom[i].len();
        let mut loops_i: Vec<Vec<i32>> = vec![vec![0]];
        if nsatom_i > 0 {
            let [start, end] = dl.loops_index[i];
            for k in start..=end {
                let glb = &dl.loops[k as usize];
                let mut lk = Vec::with_capacity(glb.len());
                lk.push(0);
                lk.extend(glb[1..].iter().rev());
                loops_i.push(lk);
            }
        } else if dc.circleindex[i].is_empty() {
            continue;
        }
        let circle0 = mod_seg_loop_cir(i, geom, di, dc, ds, &mut segment0, &mut rj);
        let [pstart, pend] = dp.patches_index[i];
        let tag = Tag {
            kind: 1,
            i: i as i32,
            j: 0,
        };
        for pj in pstart..=pend {
            let prow = &dp.patches[pj as usize];
            let mut lm = mesh_sphpat(
                c[i],
                r[i] + rp,
                &loops_i,
                &segment0,
                &circle0,
                prow,
                prow.len() - 1,
                rp,
                d,
                Some(&rj),
                Some(tag),
            )?;
            lm.vatom = vec![i as i32; lm.p.len().saturating_sub(1)];
            state.add_patch(lm)?;
        }
    }
    Ok(())
}
