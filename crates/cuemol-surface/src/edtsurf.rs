//! EDTSurf VDW/SAS/SES triangulation (Dong Xu and Yang Zhang), translated
//! from the `ProteinSurface` class as used by CueMol2.
//!
//! ```text
//! Permission to use, copy, modify, and distribute this program for
//! any purpose, with or without fee, is hereby granted, provided that
//! the notices on the head, the reference information, and this
//! copyright notice appear in all copies or substantial portions of
//! the Software. It is provided "as is" without express or implied
//! warranty.
//! ```
//!
//! Floating-point widths and evaluation order follow the C++ code so that
//! the meshes are bitwise identical.

use crate::backends::KernelMesh;
use cuemol_core::{CueMolError, Result};

const NO_RAD_TYPES: usize = 13;

const NB: [[i32; 3]; 26] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
    [1, 1, 0],
    [1, -1, 0],
    [-1, 1, 0],
    [-1, -1, 0],
    [1, 0, 1],
    [1, 0, -1],
    [-1, 0, 1],
    [-1, 0, -1],
    [0, 1, 1],
    [0, 1, -1],
    [0, -1, 1],
    [0, -1, -1],
    [1, 1, 1],
    [1, 1, -1],
    [1, -1, 1],
    [-1, 1, 1],
    [1, -1, -1],
    [-1, -1, 1],
    [-1, 1, -1],
    [-1, -1, -1],
];

/// Triangles emitted by EDTSurf's interior marching-cube case analysis for
/// each configuration of a grid cube. Corner `c` of the cube at (i, j, k) is
/// (i + c / 4, j + c / 2 % 2, k + c % 2) and sets bit `c` when it is inside.
/// The original creates vertices in the order the corners first appear here.
#[rustfmt::skip]
const CUBE_TRIANGLES: [&[[u8; 3]]; 256] = [
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[[1, 0, 2]],
    &[],
    &[],
    &[],
    &[[3, 1, 0]],
    &[],
    &[[0, 2, 3]],
    &[[2, 3, 1]],
    &[[0, 2, 3], [0, 3, 1]],
    &[],
    &[],
    &[],
    &[[4, 0, 1]],
    &[],
    &[[2, 0, 4]],
    &[],
    &[[1, 4, 2]],
    &[],
    &[],
    &[],
    &[[0, 3, 1], [0, 1, 4]],
    &[],
    &[[0, 4, 2], [0, 2, 3]],
    &[[2, 3, 1]],
    &[[2, 3, 1], [2, 1, 4]],
    &[],
    &[],
    &[],
    &[[0, 1, 5]],
    &[],
    &[],
    &[],
    &[[0, 2, 1], [0, 1, 5]],
    &[],
    &[],
    &[[5, 1, 3]],
    &[[0, 3, 5]],
    &[],
    &[[0, 2, 3]],
    &[[1, 2, 3], [1, 3, 5]],
    &[[0, 2, 3], [0, 3, 5]],
    &[],
    &[[5, 4, 0]],
    &[[1, 5, 4]],
    &[[0, 1, 5], [0, 5, 4]],
    &[],
    &[[4, 2, 0], [4, 0, 5]],
    &[[1, 5, 4]],
    &[[1, 5, 4], [1, 4, 2]],
    &[],
    &[[5, 4, 0]],
    &[[5, 4, 1], [5, 1, 3]],
    &[[5, 4, 0], [5, 0, 3]],
    &[],
    &[[3, 5, 4], [3, 4, 2], [3, 0, 5]],
    &[[3, 5, 4], [3, 4, 2], [4, 1, 2]],
    &[[5, 4, 2], [5, 2, 3]],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[[6, 2, 0]],
    &[],
    &[[0, 6, 2], [0, 2, 1]],
    &[],
    &[],
    &[],
    &[[3, 1, 0]],
    &[[3, 2, 6]],
    &[[3, 0, 6]],
    &[[2, 6, 3], [2, 3, 1]],
    &[[3, 1, 0], [3, 0, 6]],
    &[],
    &[[0, 4, 6]],
    &[],
    &[[4, 6, 0], [4, 0, 1]],
    &[[4, 6, 2]],
    &[[0, 4, 6], [0, 6, 2]],
    &[[4, 6, 2]],
    &[[4, 6, 2], [4, 2, 1]],
    &[],
    &[[0, 4, 6]],
    &[],
    &[[1, 4, 6], [1, 6, 3], [6, 0, 3]],
    &[[2, 4, 6], [2, 6, 3]],
    &[[0, 4, 6], [0, 6, 3]],
    &[[1, 4, 6], [1, 6, 3], [1, 2, 4]],
    &[[1, 4, 6], [1, 6, 3]],
    &[],
    &[],
    &[],
    &[[0, 1, 5]],
    &[],
    &[[6, 2, 0]],
    &[],
    &[[2, 1, 5], [2, 5, 6], [5, 0, 6]],
    &[],
    &[],
    &[[5, 1, 3]],
    &[[0, 3, 5]],
    &[[3, 2, 6]],
    &[[3, 0, 6]],
    &[[1, 2, 6], [1, 6, 5], [6, 3, 5]],
    &[[5, 0, 6], [5, 6, 3]],
    &[[6, 4, 5]],
    &[[5, 6, 0]],
    &[[4, 1, 5], [4, 5, 6]],
    &[[0, 1, 5], [0, 5, 6]],
    &[[6, 2, 4], [6, 4, 5]],
    &[[6, 2, 0], [6, 0, 5]],
    &[[2, 1, 5], [2, 5, 6], [2, 4, 1]],
    &[[1, 5, 6], [1, 6, 2]],
    &[[6, 4, 5]],
    &[[5, 6, 0]],
    &[[4, 1, 3], [4, 3, 6], [3, 5, 6]],
    &[[3, 5, 6], [3, 6, 0]],
    &[[2, 4, 5], [2, 5, 3], [5, 6, 3]],
    &[[3, 0, 5], [3, 5, 6]],
    &[[3, 5, 6], [1, 2, 4]],
    &[[3, 5, 6]],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[[1, 0, 2]],
    &[],
    &[],
    &[[1, 3, 7]],
    &[[1, 0, 3], [1, 3, 7]],
    &[[7, 3, 2]],
    &[[2, 7, 3], [2, 3, 0]],
    &[[2, 7, 1]],
    &[[1, 0, 2], [1, 2, 7]],
    &[],
    &[],
    &[],
    &[[4, 0, 1]],
    &[],
    &[[2, 0, 4]],
    &[],
    &[[1, 4, 2]],
    &[],
    &[],
    &[[1, 3, 7]],
    &[[7, 4, 0], [7, 0, 3], [7, 1, 4]],
    &[[7, 3, 2]],
    &[[3, 0, 4], [3, 4, 7], [4, 2, 7]],
    &[[2, 7, 1]],
    &[[7, 1, 4], [7, 4, 2]],
    &[],
    &[],
    &[[7, 5, 1]],
    &[[5, 0, 1], [5, 1, 7]],
    &[],
    &[],
    &[[7, 5, 1]],
    &[[5, 0, 2], [5, 2, 7], [2, 1, 7]],
    &[[3, 7, 5]],
    &[[3, 7, 5]],
    &[[1, 3, 7], [1, 7, 5]],
    &[[3, 7, 5], [3, 5, 0]],
    &[[3, 2, 7], [3, 7, 5]],
    &[[5, 0, 2], [5, 2, 7], [5, 3, 0]],
    &[[7, 5, 1], [7, 1, 2]],
    &[[7, 5, 0], [7, 0, 2]],
    &[[4, 5, 7]],
    &[[4, 0, 5], [4, 5, 7]],
    &[[4, 1, 7]],
    &[[4, 0, 1], [4, 1, 7]],
    &[[4, 5, 7]],
    &[[0, 5, 7], [0, 7, 2], [7, 4, 2]],
    &[[4, 1, 7]],
    &[[7, 4, 2], [7, 2, 1]],
    &[[7, 4, 5], [7, 5, 3]],
    &[[7, 4, 0], [7, 0, 3], [0, 5, 3]],
    &[[1, 3, 7], [1, 7, 4]],
    &[[0, 3, 7], [0, 7, 4]],
    &[[2, 4, 5], [2, 5, 3], [2, 7, 4]],
    &[[5, 3, 0], [7, 4, 2]],
    &[[2, 7, 4], [2, 4, 1]],
    &[[4, 2, 7]],
    &[],
    &[],
    &[],
    &[],
    &[[2, 6, 7]],
    &[[2, 0, 6], [2, 6, 7]],
    &[[2, 6, 7]],
    &[[0, 6, 7], [0, 7, 1], [7, 2, 1]],
    &[[6, 7, 3]],
    &[[6, 7, 3]],
    &[[3, 6, 7], [3, 7, 1]],
    &[[0, 6, 7], [0, 7, 1], [0, 3, 6]],
    &[[2, 6, 7], [2, 7, 3]],
    &[[6, 7, 3], [6, 3, 0]],
    &[[2, 6, 7], [2, 7, 1]],
    &[[7, 1, 0], [7, 0, 6]],
    &[[7, 6, 4]],
    &[[6, 0, 4], [6, 4, 7]],
    &[[7, 6, 4]],
    &[[6, 0, 1], [6, 1, 7], [1, 4, 7]],
    &[[7, 2, 4]],
    &[[2, 0, 4], [2, 4, 7]],
    &[[7, 2, 4]],
    &[[1, 4, 7], [1, 7, 2]],
    &[[6, 4, 7], [6, 7, 3]],
    &[[3, 0, 4], [3, 4, 7], [3, 6, 0]],
    &[[4, 1, 3], [4, 3, 6], [4, 7, 1]],
    &[[7, 1, 4], [3, 6, 0]],
    &[[7, 3, 2], [7, 2, 4]],
    &[[0, 4, 7], [0, 7, 3]],
    &[[1, 2, 4], [1, 4, 7]],
    &[[7, 1, 4]],
    &[[5, 7, 6]],
    &[[5, 7, 6]],
    &[[7, 6, 5], [7, 5, 1]],
    &[[6, 0, 1], [6, 1, 7], [6, 5, 0]],
    &[[6, 5, 7], [6, 7, 2]],
    &[[0, 5, 7], [0, 7, 2], [0, 6, 5]],
    &[[1, 2, 6], [1, 6, 5], [1, 7, 2]],
    &[[1, 7, 2], [5, 0, 6]],
    &[[6, 5, 3]],
    &[[6, 5, 3]],
    &[[5, 1, 3], [5, 3, 6]],
    &[[0, 3, 6], [0, 6, 5]],
    &[[3, 2, 6], [3, 6, 5]],
    &[[5, 3, 0], [5, 0, 6]],
    &[[1, 2, 6], [1, 6, 5]],
    &[[0, 6, 5]],
    &[[4, 5, 7], [4, 7, 6]],
    &[[5, 7, 6], [5, 6, 0]],
    &[[7, 6, 4], [7, 4, 1]],
    &[[7, 6, 0], [7, 0, 1]],
    &[[4, 5, 7], [4, 7, 2]],
    &[[7, 2, 0], [7, 0, 5]],
    &[[1, 7, 2], [1, 2, 4]],
    &[[1, 7, 2]],
    &[[6, 4, 5], [6, 5, 3]],
    &[[3, 6, 0], [3, 0, 5]],
    &[[1, 3, 6], [1, 6, 4]],
    &[[6, 0, 3]],
    &[[5, 3, 2], [5, 2, 4]],
    &[[5, 3, 0]],
    &[[2, 4, 1]],
    &[],
];

/// Distance-table half widths above this are rejected instead of allocated.
const MAX_TABLE_WIDTH: i32 = 4096;

#[derive(Clone, Copy)]
struct Voxel {
    atomid: i32,
    distance: f32,
    inout: bool,
    isbound: bool,
    isdone: bool,
}

struct Atom {
    pos: [f32; 3],
    /// Radius type index.
    detail: usize,
}

struct Vert {
    pos: [f64; 3],
    pn: [f64; 3],
    atomid: i32,
}

struct Surface {
    rasrad: [f64; NO_RAD_TYPES],
    widxz: [i32; NO_RAD_TYPES],
    /// Per radius type, the depth of the sphere above each (i, j) offset or -1.
    depty: Vec<Vec<i32>>,
    ptran: [f64; 3],
    boxlength: i32,
    proberadius: f64,
    fixsf: f64,
    scalefactor: f64,
    /// Grid size along x, y and z (`plength`, `pwidth`, `pheight`).
    dims: [i32; 3],
    cutradis: f64,
    vp: Vec<Voxel>,
    verts: Vec<Vert>,
    faces: Vec<[u32; 3]>,
}

/// Radius types 5 and 12 (hydrogens in EDTSurf) are ignored for the SES.
fn skipped(atom: &Atom, atomtype: bool) -> bool {
    atomtype && (atom.detail == 5 || atom.detail == 12)
}

impl Surface {
    fn idx(&self, i: i32, j: i32, k: i32) -> usize {
        ((i * self.dims[1] + j) * self.dims[2] + k) as usize
    }

    fn contains(&self, i: i32, j: i32, k: i32) -> bool {
        i > -1 && i < self.dims[0] && j > -1 && j < self.dims[1] && k > -1 && k < self.dims[2]
    }

    /// Scaled grid position of an atom centre, rounded as EDTSurf does.
    fn grid_point(&self, atom: &Atom) -> [i32; 3] {
        let mut c = [0; 3];
        for a in 0..3 {
            let cp = (atom.pos[a] as f64 + self.ptran[a]) * self.scalefactor;
            c[a] = (cp + 0.5) as i32;
        }
        c
    }

    fn boundingatom(&mut self, btype: bool) -> Result<()> {
        self.depty.clear();
        for i in 0..NO_RAD_TYPES {
            let tradius = if btype {
                (self.rasrad[i] + self.proberadius) * self.scalefactor + 0.5
            } else {
                self.rasrad[i] * self.scalefactor + 0.5
            };
            let sradius = tradius * tradius;
            if !(tradius < MAX_TABLE_WIDTH as f64) {
                return Err(CueMolError::invalid_argument(
                    "EDTSurf atom radius is too large for the grid",
                ));
            }
            let width = tradius as i32 + 1;
            self.widxz[i] = width;
            let mut table = Vec::with_capacity((width * width) as usize);
            for j in 0..width {
                for k in 0..width {
                    let txz = (j * j + k * k) as f64;
                    table.push(if txz > sradius {
                        -1
                    } else {
                        ((sradius - txz).sqrt() + 0.0) as i32
                    });
                }
            }
            self.depty.push(table);
        }
        Ok(())
    }

    fn initpara(&mut self, atoms: &[Atom], atomtype: bool, btype: bool) -> Result<()> {
        let fmargin = 2.5;
        let mut pmin = [100000.0; 3];
        let mut pmax = [-100000.0; 3];
        for atom in atoms.iter().filter(|a| !skipped(a, atomtype)) {
            for a in 0..3 {
                if (atom.pos[a] as f64) < pmin[a] {
                    pmin[a] = atom.pos[a] as f64;
                }
            }
            for a in 0..3 {
                if (atom.pos[a] as f64) > pmax[a] {
                    pmax[a] = atom.pos[a] as f64;
                }
            }
        }
        let margin = if btype {
            self.proberadius + fmargin
        } else {
            fmargin
        };
        for a in 0..3 {
            pmin[a] -= margin;
            pmax[a] += margin;
            self.ptran[a] = -pmin[a];
        }

        let mut extent = pmax[0] - pmin[0];
        if pmax[1] - pmin[1] > extent {
            extent = pmax[1] - pmin[1];
        }
        if pmax[2] - pmin[2] > extent {
            extent = pmax[2] - pmin[2];
        }
        self.scalefactor = (self.boxlength as f64 - 1.0) / extent;
        self.boxlength = (self.boxlength as f64 * self.fixsf / self.scalefactor) as i32;
        self.scalefactor = self.fixsf;
        let threshbox = 300.0;
        if self.boxlength as f64 > threshbox {
            let sfthresh = threshbox / self.boxlength as f64;
            self.boxlength = threshbox as i32;
            self.scalefactor *= sfthresh;
        }

        for a in 0..3 {
            let n = ((self.scalefactor * (pmax[a] - pmin[a])).ceil() + 1.0) as i32;
            self.dims[a] = n.min(self.boxlength);
        }
        if self.dims.iter().any(|&n| n < 1) {
            return Err(CueMolError::invalid_argument("EDTSurf grid is empty"));
        }
        self.boundingatom(btype)?;
        self.cutradis = self.proberadius * self.scalefactor;
        Ok(())
    }

    fn fillatom(&mut self, indx: usize, atoms: &[Atom]) {
        let [cx, cy, cz] = self.grid_point(&atoms[indx]);
        let at = atoms[indx].detail;
        let width = self.widxz[at];
        let mut nind = 0;
        for i in 0..width {
            for j in 0..width {
                let depth = self.depty[at][nind];
                nind += 1;
                if depth == -1 {
                    continue;
                }
                for ii in [-1, 1] {
                    for jj in [-1, 1] {
                        for kk in [-1, 1] {
                            let mi = ii * i;
                            let mk = kk * j;
                            for k in 0..=depth {
                                let mj = k * jj;
                                let (si, sj, sk) = (cx + mi, cy + mj, cz + mk);
                                if !self.contains(si, sj, sk) {
                                    continue;
                                }
                                let id = self.idx(si, sj, sk);
                                if !self.vp[id].inout {
                                    self.vp[id].inout = true;
                                    self.vp[id].atomid = indx as i32;
                                } else {
                                    let other = self.vp[id].atomid as usize;
                                    let [ox, oy, oz] = self.grid_point(&atoms[other]);
                                    let (ox, oy, oz) = (ox - si, oy - sj, oz - sk);
                                    if mi * mi + mj * mj + mk * mk < ox * ox + oy * oy + oz * oz {
                                        self.vp[id].atomid = indx as i32;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn fillvoxels(&mut self, atoms: &[Atom], atomtype: bool) {
        let n = (self.dims[0] * self.dims[1] * self.dims[2]) as usize;
        self.vp = vec![
            Voxel {
                atomid: -1,
                distance: -1.0,
                inout: false,
                isbound: false,
                isdone: false,
            };
            n
        ];
        for i in 0..atoms.len() {
            if !skipped(&atoms[i], atomtype) {
                self.fillatom(i, atoms);
            }
        }
        for v in &mut self.vp {
            if v.inout {
                v.isdone = true;
            }
        }
    }

    fn buildboundary(&mut self) {
        let [pl, pw, ph] = self.dims;
        for i in 0..pl {
            for j in 0..ph {
                for k in 0..pw {
                    let id = self.idx(i, k, j);
                    if !self.vp[id].inout {
                        continue;
                    }
                    let outside = NB.iter().any(|n| {
                        let (a, b, c) = (i + n[0], k + n[1], j + n[2]);
                        self.contains(a, b, c) && !self.vp[self.idx(a, b, c)].inout
                    });
                    if outside {
                        self.vp[id].isbound = true;
                    }
                }
            }
        }
    }

    /// Propagates the nearest boundary voxel over one shell of neighbours.
    fn fastoneshell(
        &mut self,
        inarray: &[[i16; 3]],
        outarray: &mut Vec<[i16; 3]>,
        boundpoint: &mut [[i16; 3]],
    ) {
        outarray.clear();
        for range in [0..6, 6..18, 18..26] {
            for &[tx, ty, tz] in inarray {
                let (tx, ty, tz) = (tx as i32, ty as i32, tz as i32);
                let bp = boundpoint[self.idx(tx, ty, tz)];
                for n in &NB[range.clone()] {
                    let (nx, ny, nz) = (tx + n[0], ty + n[1], tz + n[2]);
                    if !self.contains(nx, ny, nz) {
                        continue;
                    }
                    let id = self.idx(nx, ny, nz);
                    if !self.vp[id].inout {
                        continue;
                    }
                    let dx = nx - bp[0] as i32;
                    let dy = ny - bp[1] as i32;
                    let dz = nz - bp[2] as i32;
                    let squre = (dx * dx + dy * dy + dz * dz) as f32;
                    let v = &mut self.vp[id];
                    if !v.isdone {
                        boundpoint[id] = bp;
                        v.distance = squre.sqrt();
                        v.isdone = true;
                        v.isbound = true;
                        outarray.push([nx as i16, ny as i16, nz as i16]);
                    } else {
                        let squre = squre.sqrt();
                        if squre < v.distance {
                            boundpoint[id] = bp;
                            v.distance = squre;
                            if !v.isbound {
                                v.isbound = true;
                                outarray.push([nx as i16, ny as i16, nz as i16]);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Euclidean distance transform from the SAS boundary; the voxels at
    /// least a probe radius away form the SES solid (`isbound`).
    fn fastdistancemap(&mut self) {
        let [pl, pw, ph] = self.dims;
        let mut boundpoint = vec![[0i16; 3]; self.vp.len()];
        let mut inarray = Vec::new();
        for i in 0..pl {
            for j in 0..pw {
                for k in 0..ph {
                    let id = self.idx(i, j, k);
                    let v = &mut self.vp[id];
                    v.isdone = false;
                    if v.inout && v.isbound {
                        boundpoint[id] = [i as i16, j as i16, k as i16];
                        v.distance = 0.0;
                        v.isdone = true;
                    }
                }
            }
        }
        for i in 0..pl {
            for j in 0..pw {
                for k in 0..ph {
                    let id = self.idx(i, j, k);
                    if self.vp[id].isbound {
                        inarray.push([i as i16, j as i16, k as i16]);
                        self.vp[id].isbound = false;
                    }
                }
            }
        }

        let mut outarray = Vec::new();
        while !inarray.is_empty() {
            self.fastoneshell(&inarray, &mut outarray, &mut boundpoint);
            inarray.clear();
            for &o in &outarray {
                let id = self.idx(o[0] as i32, o[1] as i32, o[2] as i32);
                self.vp[id].isbound = false;
                if self.vp[id].distance as f64 <= 1.02 * self.cutradis {
                    inarray.push(o);
                }
            }
        }

        let mut cutsf = self.scalefactor - 0.5;
        if cutsf < 0.0 {
            cutsf = 0.0;
        }
        let cut = self.cutradis - 0.50 / (0.1 + cutsf);
        for id in 0..self.vp.len() {
            let v = self.vp[id];
            self.vp[id].isbound = false;
            if v.inout && (!v.isdone || (v.isdone && v.distance as f64 >= cut)) {
                self.vp[id].isbound = true;
                if v.isdone {
                    let [bx, by, bz] = boundpoint[id];
                    let src = self.idx(bx as i32, by as i32, bz as i32);
                    self.vp[id].atomid = self.vp[src].atomid;
                }
            }
        }
    }

    fn vertex(&mut self, vertseq: &mut [i32], p: [i32; 3]) -> u32 {
        let id = self.idx(p[0], p[1], p[2]);
        if vertseq[id] == -1 {
            vertseq[id] = self.verts.len() as i32;
            self.verts.push(Vert {
                pos: [p[0] as f64, p[1] as f64, p[2] as f64],
                pn: [0.0; 3],
                atomid: -1,
            });
        }
        vertseq[id] as u32
    }

    /// Caps the grid face where `axis` equals `fixed`; `flip` reverses the
    /// triangle orientation.
    fn boundary_faces(&mut self, vertseq: &mut [i32], axis: usize, fixed: i32, flip: bool) {
        let (a1, a2) = match axis {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        for p in 0..self.dims[a1] - 1 {
            for q in 0..self.dims[a2] - 1 {
                let corner = |dp: i32, dq: i32| {
                    let mut c = [0; 3];
                    c[axis] = fixed;
                    c[a1] = p + dp;
                    c[a2] = q + dq;
                    c
                };
                let c = [corner(0, 0), corner(1, 0), corner(1, 1), corner(0, 1)];
                let done = c.map(|c| self.vp[self.idx(c[0], c[1], c[2])].isdone);
                let tp: Vec<[i32; 3]> = if done.iter().all(|&d| d) {
                    c.to_vec()
                } else if let Some(r) =
                    (0..4).find(|&r| done[r] && done[(r + 1) % 4] && done[(r + 2) % 4])
                {
                    vec![c[r], c[(r + 1) % 4], c[(r + 2) % 4]]
                } else {
                    continue;
                };
                let ids: Vec<u32> = tp.iter().map(|&t| self.vertex(vertseq, t)).collect();
                for s in 1..ids.len() - 1 {
                    self.faces.push(if flip {
                        [ids[0], ids[s + 1], ids[s]]
                    } else {
                        [ids[0], ids[s], ids[s + 1]]
                    });
                }
            }
        }
    }

    /// Marching cubes over `isdone`; `stype` 1 = VDW, 3 = SAS, 4 = SES.
    fn marchingcube(&mut self, stype: i32) {
        let [pl, pw, ph] = self.dims;
        if stype == 4 {
            for v in &mut self.vp {
                v.isdone = v.isbound;
                v.isbound = false;
            }
        } else {
            for v in &mut self.vp {
                v.isbound = false;
            }
        }
        let mut vertseq = vec![-1i32; self.vp.len()];
        self.verts.clear();
        self.faces.clear();

        self.boundary_faces(&mut vertseq, 0, 0, true);
        self.boundary_faces(&mut vertseq, 1, 0, false);
        self.boundary_faces(&mut vertseq, 2, 0, true);
        self.boundary_faces(&mut vertseq, 0, pl - 1, false);
        self.boundary_faces(&mut vertseq, 1, pw - 1, true);
        self.boundary_faces(&mut vertseq, 2, ph - 1, false);

        for i in 0..pl - 1 {
            for j in 0..pw - 1 {
                for k in 0..ph - 1 {
                    let corner = |c: u8| {
                        let c = c as i32;
                        [i + (c >> 2), j + ((c >> 1) & 1), k + (c & 1)]
                    };
                    let mut config = 0;
                    for c in 0..8 {
                        let [x, y, z] = corner(c);
                        if self.vp[self.idx(x, y, z)].isdone {
                            config |= 1 << c;
                        }
                    }
                    for tri in CUBE_TRIANGLES[config] {
                        let mut face = [0; 3];
                        for (f, &c) in face.iter_mut().zip(tri) {
                            *f = self.vertex(&mut vertseq, corner(c));
                        }
                        self.faces.push(face);
                    }
                }
            }
        }

        for n in 0..self.verts.len() {
            let [x, y, z] = self.verts[n].pos.map(|v| v as i32);
            self.verts[n].atomid = self.vp[self.idx(x, y, z)].atomid;
        }
    }

    fn computenorm(&mut self) {
        for v in &mut self.verts {
            v.pn = [0.0; 3];
        }
        for f in 0..self.faces.len() {
            let [a, b, c] = self.faces[f].map(|v| v as usize);
            let (p1, p2, p3) = (self.verts[a].pos, self.verts[b].pos, self.verts[c].pos);
            let p12 = [p2[0] - p1[0], p2[1] - p1[1], p2[2] - p1[2]];
            let p13 = [p3[0] - p1[0], p3[1] - p1[1], p3[2] - p1[2]];
            let pn = [
                p12[1] * p13[2] - p12[2] * p13[1],
                p12[2] * p13[0] - p12[0] * p13[2],
                p12[0] * p13[1] - p12[1] * p13[0],
            ];
            let area = 0.5 * (pn[0] * pn[0] + pn[1] * pn[1] + pn[2] * pn[2]).sqrt();
            let fpn = pn.map(|v| 0.5 * v / area);
            for v in [a, b, c] {
                for axis in 0..3 {
                    self.verts[v].pn[axis] += fpn[axis];
                }
            }
        }
        for v in &mut self.verts {
            let pn = v.pn;
            let pnorm = (pn[0] * pn[0] + pn[1] * pn[1] + pn[2] * pn[2]).sqrt();
            v.pn = if pnorm == 0.0 {
                [0.0; 3]
            } else {
                pn.map(|c| c / pnorm)
            };
        }
    }

    /// Laplacian smoothing followed by an inward shift along the normals.
    fn laplaciansmooth(&mut self, numiter: usize) {
        // Mesh neighbours are corners of a shared grid cube, so at most 26.
        const MAX_DEGREE: usize = 26;
        let n = self.verts.len();
        let mut degree = vec![0usize; n];
        let mut neighbors = vec![0u32; n * MAX_DEGREE];
        for &[a, b, c] in &self.faces {
            for (v, w) in [(a, b), (a, c), (b, a), (b, c), (c, a), (c, b)] {
                let v = v as usize;
                let list = &mut neighbors[v * MAX_DEGREE..v * MAX_DEGREE + degree[v]];
                if !list.contains(&w) {
                    neighbors[v * MAX_DEGREE + degree[v]] = w;
                    degree[v] += 1;
                }
            }
        }

        let wt = 1.00;
        let wt2 = 0.50;
        let outwt = 0.75 / (self.scalefactor + 3.5);
        let mut tps = vec![[0.0; 3]; n];
        for _ in 0..numiter {
            for i in 0..n {
                let deg = degree[i];
                if deg < 3 {
                    tps[i] = self.verts[i].pos;
                    continue;
                }
                let w = if deg == 3 || deg == 4 { wt2 } else { wt };
                let mut t = [0.0; 3];
                for &nb in &neighbors[i * MAX_DEGREE..i * MAX_DEGREE + deg] {
                    for axis in 0..3 {
                        t[axis] += self.verts[nb as usize].pos[axis];
                    }
                }
                let divisor = (w + deg as f64) as f32 as f64;
                for axis in 0..3 {
                    t[axis] += w * self.verts[i].pos[axis];
                    t[axis] /= divisor;
                }
                tps[i] = t;
            }
            for i in 0..n {
                self.verts[i].pos = tps[i];
            }
            self.computenorm();
            // EDTSurf reads an uninitialized in/out flag here; CueMol clears
            // it, so every vertex moves against its normal.
            let shift = -1.0 * outwt;
            for v in &mut self.verts {
                for axis in 0..3 {
                    v.pos[axis] += shift * v.pn[axis];
                }
            }
        }
    }
}

/// Builds an EDTSurf mesh; `surface_type` 0 = VDW, 1 = SAS, 2 = SES and
/// `fixsf` is the grid scale (voxels per Å).
pub(crate) fn edtsurf_mesh(
    xyzr: &[f64],
    fixsf: f64,
    probe: f64,
    surface_type: u32,
) -> Result<KernelMesh> {
    if xyzr.len() < 4 {
        return Err(CueMolError::invalid_argument(
            "EDTSurf needs at least one atom",
        ));
    }
    if !xyzr.iter().all(|v| v.is_finite()) || !fixsf.is_finite() || !probe.is_finite() {
        return Err(CueMolError::invalid_argument(
            "EDTSurf input must be finite",
        ));
    }
    let mut radii: Vec<f64> = Vec::new();
    let mut atoms = Vec::with_capacity(xyzr.len() / 4);
    for a in xyzr.chunks_exact(4) {
        let detail = match radii.iter().position(|&r| r == a[3]) {
            Some(index) => index,
            None => {
                radii.push(a[3]);
                radii.len() - 1
            }
        };
        if radii.len() > NO_RAD_TYPES {
            return Err(CueMolError::invalid_argument(
                "too many EDTSurf radius types",
            ));
        }
        atoms.push(Atom {
            pos: [a[0] as f32, a[1] as f32, a[2] as f32],
            detail,
        });
    }

    let mut surface = Surface {
        rasrad: std::array::from_fn(|i| radii.get(i).copied().unwrap_or(1.7)),
        widxz: [0; NO_RAD_TYPES],
        depty: Vec::new(),
        ptran: [0.0; 3],
        boxlength: 128,
        proberadius: probe,
        fixsf,
        scalefactor: 1.0,
        dims: [0; 3],
        cutradis: 0.0,
        vp: Vec::new(),
        verts: Vec::new(),
        faces: Vec::new(),
    };
    let ses = surface_type == 2;
    surface.initpara(&atoms, ses, surface_type != 0)?;
    surface.fillvoxels(&atoms, ses);
    surface.buildboundary();
    if ses {
        surface.fastdistancemap();
    }
    surface.marchingcube(match surface_type {
        0 => 1,
        1 => 3,
        _ => 4,
    });
    surface.vp = Vec::new();
    surface.laplaciansmooth(1);
    surface.computenorm();

    let mut vertices = Vec::with_capacity(surface.verts.len() * 6);
    let mut owners = Vec::with_capacity(surface.verts.len());
    for v in &surface.verts {
        for axis in 0..3 {
            vertices.push((v.pos[axis] / surface.scalefactor - surface.ptran[axis]) as f32 as f64);
        }
        for axis in 0..3 {
            vertices.push(v.pn[axis] as f32 as f64);
        }
        owners.push(if v.atomid < 0 {
            u32::MAX
        } else {
            v.atomid as u32
        });
    }
    let triangles = surface.faces.iter().flatten().copied().collect();
    Ok((vertices, triangles, 3 + surface_type, owners))
}
