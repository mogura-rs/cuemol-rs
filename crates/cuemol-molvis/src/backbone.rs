pub(super) mod cartoon;
mod smoothing;

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Face {
    Unknown,
    Front,
    Back,
    Side1,
    Side2,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SectionVertex {
    pub xy: [f64; 2],
    pub normal: [f64; 2],
    pub face: Face,
}

#[derive(Clone, Debug)]
pub(super) struct Profile {
    pub vertices: std::sync::Arc<[SectionVertex]>,
    pub size: [f64; 2],
    pub shape: SectionShape,
    fancy_divisions: Option<(usize, usize)>,
}

impl Profile {
    pub fn new(width: f64, thickness: f64, shape: SectionShape, sharp: f64, detail: usize) -> Self {
        use std::f64::consts::{FRAC_PI_2, PI, TAU};
        let x = thickness.max(1.0e-8);
        let y = width.max(1.0e-8);
        let detail = detail.max(3);
        let sharp = sharp.clamp(0.0, 1.0);
        let mut vertices = Vec::new();
        let mut fancy_divisions = None;
        let mut add = |x, y, nx, ny, face| {
            vertices.push(SectionVertex {
                xy: [x, y],
                normal: [nx, ny],
                face,
            });
        };
        match shape {
            SectionShape::Elliptical => {
                let ratio = y / x;
                let theta = |phi: f64| {
                    let turns = (phi / PI).floor();
                    let part = phi % PI;
                    turns * PI
                        + if part < FRAC_PI_2 {
                            (ratio * part.tan()).atan()
                        } else {
                            PI - (ratio * (PI - part).tan()).atan()
                        }
                };
                let limit = TAU / detail as f64 + 0.001;
                let mut angles = Vec::new();
                for i in 0..detail {
                    let start = theta(i as f64 * TAU / detail as f64);
                    let end = theta((i + 1) as f64 * TAU / detail as f64);
                    angles.push(start);
                    if end - start > limit {
                        let count = ((end - start) / limit).ceil() as usize;
                        for j in 1..count {
                            let angle = start + j as f64 * (end - start) / count as f64;
                            if angle < TAU {
                                angles.push(angle);
                            }
                        }
                    }
                }
                angles.sort_by(f64::total_cmp);
                angles.dedup();
                for angle in angles {
                    let (sin, cos) = angle.sin_cos();
                    let len = (cos * cos * ratio * ratio + sin * sin).sqrt();
                    add(
                        cos * x,
                        sin * y,
                        cos * ratio / len,
                        sin / len,
                        Face::Unknown,
                    );
                }
            }
            SectionShape::Rectangular => {
                let delta = 4.0 * (x + y) / detail as f64;
                let nx = (2.0 * x / delta).floor().max(1.0) as usize;
                let ny = (2.0 * y / delta).floor().max(1.0) as usize;
                for j in 0..=nx {
                    add(x - 2.0 * x * j as f64 / nx as f64, y, 0.0, 1.0, Face::Side2);
                }
                for j in 0..=ny {
                    add(
                        -x,
                        y - 2.0 * y * j as f64 / ny as f64,
                        -1.0,
                        0.0,
                        Face::Front,
                    );
                }
                for j in 0..=nx {
                    add(
                        -x + 2.0 * x * j as f64 / nx as f64,
                        -y,
                        0.0,
                        -1.0,
                        Face::Side1,
                    );
                }
                for j in 0..=ny {
                    add(x, -y + 2.0 * y * j as f64 / ny as f64, 1.0, 0.0, Face::Back);
                }
            }
            SectionShape::RoundSquare => {
                let radius = x.min(y) * (1.0 - sharp);
                let ax = x - radius;
                let ay = y - radius;
                let delta = (2.0 * (ax + ay) + radius * TAU) / detail as f64;
                let nx = (2.0 * ax / delta).floor().max(1.0) as usize;
                let ny = (2.0 * ay / delta).floor().max(1.0) as usize;
                let nr = (radius * FRAC_PI_2 / delta).floor().max(1.0) as usize * 2;
                for quadrant in 0..4 {
                    let sx = if quadrant == 0 || quadrant == 3 {
                        1.0
                    } else {
                        -1.0
                    };
                    let sy = if quadrant < 2 { 1.0 } else { -1.0 };
                    let face = if quadrant < 2 {
                        Face::Side2
                    } else {
                        Face::Side1
                    };
                    for j in 0..nr {
                        let angle = (quadrant as f64 + j as f64 / nr as f64) * FRAC_PI_2;
                        let (sin, cos) = angle.sin_cos();
                        add(
                            sx * ax + cos * radius,
                            sy * ay + sin * radius,
                            cos,
                            sin,
                            face,
                        );
                    }
                    if quadrant % 2 == 0 {
                        for j in 0..nx {
                            add(
                                sx * (ax - 2.0 * ax * j as f64 / nx as f64),
                                sy * y,
                                0.0,
                                sy,
                                face,
                            );
                        }
                    } else {
                        for j in 0..ny {
                            add(
                                sx * x,
                                sy * (ay - 2.0 * ay * j as f64 / ny as f64),
                                sx,
                                0.0,
                                if quadrant == 1 {
                                    Face::Front
                                } else {
                                    Face::Back
                                },
                            );
                        }
                    }
                }
            }
            SectionShape::Fancy => {
                let theta = PI * sharp;
                let ax = x * theta.sin();
                let by = (y - x * (1.0 + theta.cos())).max(0.0);
                let cy = by + x * theta.cos();
                let phi = 2.0 * (PI - theta);
                let wx = phi * x;
                let wy = 2.0 * by;
                let delta = (2.0 * wy + 2.0 * wx).max(1.0e-8) / detail as f64;
                let nx = (wx / delta).floor().max(4.0) as usize * 2;
                let ny = (wy / delta).floor().max(1.0) as usize;
                fancy_divisions = Some((nx, ny));
                for j in 0..=nx {
                    let angle = theta - FRAC_PI_2 + j as f64 * phi / nx as f64;
                    let (sin, cos) = angle.sin_cos();
                    add(cos * x, sin * x + cy, cos, sin, Face::Side2);
                }
                for j in 0..=ny {
                    add(-ax, by - wy * j as f64 / ny as f64, -1.0, 0.0, Face::Front);
                }
                for j in 0..=nx {
                    let angle = theta + FRAC_PI_2 + j as f64 * phi / nx as f64;
                    let (sin, cos) = angle.sin_cos();
                    add(cos * x, sin * x - cy, cos, sin, Face::Side1);
                }
                for j in 0..=ny {
                    add(ax, -by + wy * j as f64 / ny as f64, 1.0, 0.0, Face::Back);
                }
            }
        }
        Self {
            vertices: vertices.into(),
            size: [x, y],
            shape,
            fancy_divisions,
        }
    }

    pub fn for_section(ss: Secondary, options: &MoleculeRenderOptions) -> Self {
        let scale = (options.bond_radius * 4.0).clamp(0.25, 0.8) as f64;
        let (width, thickness, _) =
            section_params(ss, scale, &options.ribbon_sections, &options.styled_ribbon);
        let detail = match ss {
            Secondary::Helix => options.styled_ribbon.helix_detail,
            Secondary::Sheet => options.styled_ribbon.sheet_detail,
            Secondary::Coil => options.styled_ribbon.coil_detail,
        }
        .or(options.ribbon_radial_detail)
        .unwrap_or(16) as usize;
        Self::new(
            width,
            thickness,
            options.styled_ribbon.shape_for(ss),
            options.styled_ribbon.sharp_for(ss),
            detail,
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Scale {
    pub t: f64,
    pub xy: [f64; 2],
    pub derivative: [f64; 2],
}

pub(super) fn junction(
    from: &Profile,
    to: &Profile,
    reverse: bool,
    detail: usize,
    spec: RibbonJunction,
) -> Vec<Scale> {
    let detail = detail.max(2);
    let base = if reverse { from.size } else { to.size };
    let gamma = spec.gamma.max(0.1);
    let mut result = Vec::new();
    let smooth = |t: f64, guard: bool| {
        let u = 2.0
            * if guard {
                t.min(1.0 - t).max(1.0e-4)
            } else {
                t.min(1.0 - t)
            };
        let rho = 0.5 * u.powf(gamma);
        (
            if t <= 0.5 { rho } else { 1.0 - rho },
            gamma * u.powf(gamma - 1.0),
        )
    };
    if spec.kind != RibbonJunctionType::Arrow {
        for i in 0..detail {
            let t = i as f64 / (detail - 1) as f64;
            let (rho, drho) = smooth(t, false);
            let mut scale = Scale {
                t,
                xy: [0.0; 2],
                derivative: [0.0; 2],
            };
            for axis in 0..2 {
                let range = to.size[axis] - from.size[axis];
                scale.xy[axis] = (from.size[axis] + rho * range) / base[axis];
                scale.derivative[axis] = drho * range / base[axis];
            }
            if spec.kind == RibbonJunctionType::Flat {
                scale.xy[1] = 1.0;
                scale.derivative[1] = 0.0;
            }
            result.push(scale);
        }
    } else {
        let end = if reverse { to.size } else { from.size };
        let basw = spec.base_width.clamp(0.0, 1.0 - 2.0 / detail as f64);
        let body = (detail as f64 * basw).floor() as usize;
        let head = detail - body;
        result.push(Scale {
            t: 0.0,
            xy: [1.0; 2],
            derivative: [0.0; 2],
        });
        for i in 1..=body {
            result.push(Scale {
                t: i as f64 / body as f64 * basw,
                xy: [1.0; 2],
                derivative: [0.0; 2],
            });
        }
        for i in 0..head {
            let t = i as f64 / (head - 1) as f64;
            let (rho_x, drho_x) = smooth(t, true);
            let power = (1.0 - t).powf(gamma);
            let rho_y = 1.0 - power;
            let drho_y = gamma
                * if 1.0 - t > 1.0e-4 {
                    power / (1.0 - t)
                } else {
                    1.0e-4_f64.powf(gamma - 1.0)
                };
            let range_x = end[0] - base[0];
            let range_y = end[1] - base[1] * spec.arrow_width;
            result.push(Scale {
                t: t * (1.0 - basw) + basw,
                xy: [
                    (base[0] + rho_x * range_x) / base[0],
                    (base[1] * spec.arrow_width + rho_y * range_y) / base[1],
                ],
                derivative: [drho_x * range_x / base[0], drho_y * range_y / base[1]],
            });
        }
        if !reverse {
            result.reverse();
            for entry in &mut result {
                entry.t = 1.0 - entry.t;
            }
        }
    }
    result
}

pub(super) struct Axis {
    pub spline: CubicSpline,
    pub normal: CubicSpline,
    pub flipped: Vec<bool>,
}

pub(super) fn pivot_binormal(points: &[MainchainPoint], i: usize) -> Vector4 {
    if let Some(co) = points[i].carbonyl.filter(|co| co.length3d() >= 1.0e-4) {
        return co.scale(1.0 / co.length3d());
    }
    if points.len() < 3 {
        return Vector4::new3(1.0, 0.0, 0.0);
    }
    let mid = i.clamp(1, points.len() - 2);
    let a = points[mid].pos - points[mid - 1].pos;
    let b = points[mid + 1].pos - points[mid].pos;
    normalize_or(a.cross(b), Vector4::new3(1.0, 0.0, 0.0))
}

fn orient_binormal(
    binormal: Vector4,
    tangent: Vector4,
    previous: Option<(Vector4, Vector4)>,
) -> (Vector4, bool) {
    let Some((prev_normal, prev_tangent)) = previous else {
        return (binormal, false);
    };
    let tangent = normalize_or(tangent, prev_tangent);
    let prev_tangent = normalize_or(prev_tangent, tangent);
    let axis = tangent.cross(prev_tangent);
    let transported = if axis.length3d() > 1.0e-4 {
        let axis = axis.scale(1.0 / axis.length3d());
        let cos = tangent.dot3d(prev_tangent).clamp(-1.0, 1.0);
        let sin = (1.0 - cos * cos).sqrt();
        // Matrix3D::makeRotMat uses the row-vector sign convention.
        prev_normal.scale(cos) - axis.cross(prev_normal).scale(sin)
            + axis.scale(axis.dot3d(prev_normal) * (1.0 - cos))
    } else {
        prev_normal
    };
    let flipped = binormal.dot3d(transported) < 0.0;
    (
        if flipped {
            binormal.scale(-1.0)
        } else {
            binormal
        },
        flipped,
    )
}

impl Axis {
    pub fn new(points: &[MainchainPoint], options: &MoleculeRenderOptions) -> Self {
        let mut positions = points.iter().map(|point| point.pos).collect::<Vec<_>>();
        if positions.len() >= 3 {
            for i in 1..positions.len() - 1 {
                let smooth = if options.uniform_backbone {
                    options.anchor_weight
                } else {
                    options
                        .ribbon_smoothing
                        .for_ss(points[i].ss)
                        .max(options.anchor_weight)
                }
                .clamp(0.0, 1.0);
                positions[i] = points[i].pos.scale(1.0 - smooth)
                    + (points[i - 1].pos + points[i + 1].pos).scale(0.5 * smooth);
            }
        }
        let spline = CubicSpline::generate(&positions).expect("nonempty backbone");
        let mut previous = None;
        let mut normals = Vec::with_capacity(points.len());
        let mut flipped = Vec::with_capacity(points.len());
        for i in 0..points.len() {
            let sample = spline.interpolate(i as f64);
            let (binormal, flip) =
                orient_binormal(pivot_binormal(points, i), sample.tangent, previous);
            normals.push(sample.position + binormal);
            flipped.push(flip);
            previous = Some((binormal, sample.tangent));
        }
        Self {
            spline,
            normal: CubicSpline::generate(&normals).expect("nonempty backbone"),
            flipped,
        }
    }

    pub fn sample(&self, par: f64, scale: Scale, ss: Secondary) -> Ring {
        let axis = self.spline.interpolate(par);
        let e2 = self.normal.interpolate(par).position - axis.position;
        let tangent = normalize_or(axis.tangent, Vector4::new3(0.0, 0.0, 1.0));
        let e1 = normalize_or(e2.cross(tangent), perpendicular(tangent));
        let e2 = if e2.cross(tangent).length3d() > 1.0e-8 {
            e2
        } else {
            tangent.cross(e1)
        };
        let low = (par.floor().max(0.0) as usize).min(self.flipped.len() - 1);
        let high = (par.ceil().max(0.0) as usize).min(self.flipped.len() - 1);
        Ring {
            center: axis.position,
            tangent: axis.tangent,
            e1,
            e2,
            scale,
            par,
            ss,
            flipped: self.flipped[low] || self.flipped[high],
            normal_offset: Vector4::new3(0.0, 0.0, 0.0),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Ring {
    pub center: Vector4,
    pub tangent: Vector4,
    pub e1: Vector4,
    pub e2: Vector4,
    pub scale: Scale,
    pub par: f64,
    pub ss: Secondary,
    pub flipped: bool,
    pub normal_offset: Vector4,
}

pub(super) struct Strip {
    pub rings: Vec<Ring>,
    pub profile: Profile,
    pub start_cap: TubeCapType,
    pub end_cap: TubeCapType,
    pub unscaled_start_cap: bool,
    pub extra_start_cap: bool,
    pub start_cap_next_color: bool,
}

fn selected_ranges(points: &[MainchainPoint]) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = None;
    for (i, selected) in points.iter().map(|p| p.selected).chain([false]).enumerate() {
        if selected {
            start.get_or_insert(i);
        } else if let Some(start) = start.take() {
            ranges.push(start..i);
        }
    }
    ranges
}

fn segment_end(par: f64, points: &[MainchainPoint]) -> (f64, bool, bool) {
    let low = par.floor() as isize;
    let high = par.ceil() as isize;
    let at = |i: isize| usize::try_from(i).ok().and_then(|i| points.get(i));
    let mut rho = par - par.floor();
    let mut first = false;
    let mut second = false;
    if let (Some(previous), Some(current)) = (at(low - 1), at(low)) {
        if !previous.pivot_selected {
            first = true;
            second |= !current.pivot_selected;
        }
    }
    if let (Some(next), Some(current)) = (at(high + 1), at(high)) {
        if !next.pivot_selected {
            second = true;
            if rho.abs() < 1.0e-8 {
                rho = 1.0;
            }
            first |= !current.pivot_selected;
        }
    }
    (rho, first, second)
}

fn visible_cap(
    cap: TubeCapType,
    par: f64,
    points: &[MainchainPoint],
    options: &MoleculeRenderOptions,
) -> TubeCapType {
    let (_, first, second) = segment_end(par, points);
    if options.segment_end_fade && (first || second) {
        TubeCapType::None
    } else {
        cap
    }
}

fn tube_strips(
    points: &[MainchainPoint],
    options: &MoleculeRenderOptions,
    axis: &Axis,
) -> Vec<Strip> {
    let detail = options.ribbon_axial_detail.unwrap_or(6).max(1) as usize;
    let profile = Profile::for_section(Secondary::Coil, options);
    selected_ranges(points)
        .into_iter()
        .filter(|r| r.len() > 1)
        .map(|range| {
            let count = (range.len() - 1) * detail;
            let rings = (0..=count)
                .map(|j| {
                    let par = range.start as f64 + j as f64 / detail as f64;
                    let mut ring = axis.sample(
                        par,
                        Scale {
                            t: 0.0,
                            xy: [1.0; 2],
                            derivative: [0.0; 2],
                        },
                        Secondary::Coil,
                    );
                    let tangent = normalize_or(ring.tangent, Vector4::new3(0.0, 0.0, 1.0));
                    ring.e2 = normalize_or(
                        ring.e2 - tangent.scale(tangent.dot3d(ring.e2)),
                        perpendicular(tangent),
                    );
                    ring.e1 = ring.e2.cross(tangent);
                    ring
                })
                .collect();
            Strip {
                rings,
                profile: profile.clone(),
                start_cap: visible_cap(options.start_cap_type, range.start as f64, points, options),
                end_cap: visible_cap(
                    options.end_cap_type,
                    (range.end - 1) as f64,
                    points,
                    options,
                ),
                unscaled_start_cap: false,
                extra_start_cap: false,
                start_cap_next_color: false,
            }
        })
        .collect()
}

pub(super) fn ribbon_strips(
    points: &[MainchainPoint],
    options: &MoleculeRenderOptions,
) -> Vec<Strip> {
    let axis = Axis::new(points, options);
    if options.uniform_backbone {
        return tube_strips(points, options, &axis);
    }
    let coil = Profile::for_section(Secondary::Coil, options);
    let helix = Profile::for_section(Secondary::Helix, options);
    let sheet = Profile::for_section(Secondary::Sheet, options);
    let detail = options.ribbon_axial_detail.unwrap_or(8).max(2) as usize;
    let mut strips: Vec<Strip> = Vec::with_capacity(points.len());
    let mut partition_next = false;
    for (i, point) in points.iter().enumerate() {
        if !point.selected {
            partition_next = false;
            continue;
        }
        let segment_start = i == 0 || !points[i - 1].selected;
        let segment_last = i + 1 == points.len() || !points[i + 1].selected;
        let start = if i == 0 { 0.0 } else { i as f64 - 0.5 };
        let end = if i + 1 == points.len() {
            i as f64
        } else {
            i as f64 + 0.5
        };
        let ss = if options.uniform_backbone {
            Secondary::Coil
        } else {
            point.ss
        };
        let profile = match ss {
            Secondary::Coil => coil.clone(),
            Secondary::Helix => helix.clone(),
            Secondary::Sheet => sheet.clone(),
        };
        let marker = point.secondary_marker;
        let tail = ss != Secondary::Coil
            && (marker == b's' || (marker == 0 && (i == 0 || points[i - 1].ss != ss)));
        let head = ss != Secondary::Coil
            && !tail
            && (marker == b'e'
                || (marker == 0 && (i + 1 == points.len() || points[i + 1].ss != ss)));
        let scales = if tail || head {
            let spec = match (ss, head) {
                (Secondary::Helix, true) => options.helix_head,
                (Secondary::Helix, false) => options.helix_tail,
                (Secondary::Sheet, true) => options.sheet_head,
                _ => options.sheet_tail,
            };
            if head {
                junction(&profile, &coil, true, detail + 1, spec)
            } else {
                junction(&coil, &profile, false, detail + 1, spec)
            }
        } else {
            let count = ((end - start) * detail as f64).floor().max(1.0) as usize;
            (0..=count)
                .map(|j| Scale {
                    t: j as f64 / count as f64,
                    xy: [1.0; 2],
                    derivative: [0.0; 2],
                })
                .collect()
        };
        let rings: Vec<Ring> = scales
            .into_iter()
            .map(|scale| {
                let par = start + scale.t * if tail || head { 1.0 } else { end - start };
                let mut ring = axis.sample(par, scale, ss);
                if options.uniform_backbone {
                    let tangent = normalize_or(ring.tangent, Vector4::new3(0.0, 0.0, 1.0));
                    ring.e2 = normalize_or(
                        ring.e2 - tangent.scale(tangent.dot3d(ring.e2)),
                        perpendicular(tangent),
                    );
                    ring.e1 = ring.e2.cross(tangent);
                }
                ring
            })
            .collect();
        let junction_flat = if ss == Secondary::Helix {
            if head {
                options.helix_head.kind
            } else {
                options.helix_tail.kind
            }
        } else if head {
            options.sheet_head.kind
        } else {
            options.sheet_tail.kind
        } == RibbonJunctionType::Flat;
        let partition = profile.shape != coil.shape || junction_flat;
        let partition_start = partition_next || tail && partition;
        let start_cap = if segment_start {
            if ss == Secondary::Coil {
                options.start_cap_type
            } else {
                TubeCapType::Flat
            }
        } else if partition_start {
            TubeCapType::Flat
        } else {
            TubeCapType::None
        };
        if partition_start && !segment_start {
            if let Some(last) = strips.last_mut() {
                last.end_cap = TubeCapType::Flat;
            }
        }
        partition_next = head && partition;
        let start_cap = if segment_start && !(head || tail) {
            visible_cap(start_cap, rings[0].par, points, options)
        } else {
            start_cap
        };
        let end_cap = if segment_last {
            visible_cap(
                if ss == Secondary::Coil {
                    options.end_cap_type
                } else {
                    TubeCapType::Flat
                },
                rings.last().unwrap().par,
                points,
                options,
            )
        } else {
            TubeCapType::None
        };
        strips.push(Strip {
            rings,
            profile,
            start_cap,
            end_cap,
            unscaled_start_cap: segment_start && (head || tail),
            extra_start_cap: segment_start && (head || tail) && partition_start,
            start_cap_next_color: !segment_start && partition_start,
        });
    }
    strips
}

pub(super) fn mesh(
    strips: &[Strip],
    points: &[MainchainPoint],
    paint: Option<&[Option<[f32; 4]>]>,
    width_scales: &[f64],
    options: &MoleculeRenderOptions,
    subdiv: usize,
) -> RenderMesh {
    let mut mesh = RenderMesh::new();
    let owner_at = |ring: &Ring| {
        let pivot = (ring.par.floor()
            + if ring.par - ring.par.floor() > 0.5 {
                1.
            } else {
                0.
            })
        .max(0.) as usize;
        points
            .get(pivot.min(points.len().saturating_sub(1)))
            .map(|point| point.atom_index as u32)
    };
    let color_at = |ring: &Ring| {
        let par = if options.smooth_color {
            ring.par
        } else {
            ring.par.floor() + if ring.par.fract() > 0.5 { 1.0 } else { 0.0 }
        };
        let index = (par * subdiv as f64).max(0.0);
        let color = paint.and_then(|paint| {
            let low = (index as usize).min(paint.len().saturating_sub(1));
            let a = paint.get(low).copied().flatten()?;
            if !options.smooth_color {
                return Some(a);
            }
            let b = paint.get(low + 1).copied().flatten().unwrap_or(a);
            let f = index.fract() as f32;
            Some(std::array::from_fn(|i| a[i] + (b[i] - a[i]) * f))
        });
        let mut color = color.unwrap_or_else(|| {
            section_params(
                ring.ss,
                1.0,
                &options.ribbon_sections,
                &options.styled_ribbon,
            )
            .2
        });
        if options.segment_end_fade && !options.cartoon {
            let (rho, first, second) = segment_end(ring.par, points);
            let alpha = |i: usize, transparent: bool| {
                if transparent {
                    0.0
                } else {
                    paint
                        .and_then(|p| p.get(i * subdiv))
                        .copied()
                        .flatten()
                        .map_or(color[3], |c| c[3])
                }
            };
            let a = alpha(ring.par.floor().max(0.0) as usize, first);
            let b = alpha(ring.par.ceil().max(0.0) as usize, second);
            color[3] = if options.smooth_color {
                a * (1.0 - rho as f32) + b * rho as f32
            } else if rho > 0.5 {
                b
            } else {
                a
            };
        }
        color
    };
    for strip in strips {
        let radial = strip.profile.vertices.len();
        let mut last = None;
        for (ri, ring) in strip.rings.iter().enumerate() {
            let base = mesh.vertices.len();
            let putty = width_scales
                .get((ring.par.max(0.0) * subdiv as f64).round() as usize)
                .copied()
                .unwrap_or(1.0);
            let scale = ring.scale;
            let color = color_at(ring);
            let pick_atom_index = owner_at(ring);
            for vertex in strip.profile.vertices.iter() {
                let [x, y] = vertex.xy;
                let [nx, ny] = vertex.normal;
                let offset =
                    ring.e1.scale(x * scale.xy[0] * putty) + ring.e2.scale(y * scale.xy[1] * putty);
                let lateral = ring.e1.scale(nx * scale.xy[1]) + ring.e2.scale(ny * scale.xy[0]);
                let derivative = if options.cartoon {
                    -(nx.abs() * strip.profile.size[0] * scale.derivative[0]
                        + ny.abs() * strip.profile.size[1] * scale.derivative[1])
                } else {
                    nx * scale.xy[1] * x * scale.derivative[0]
                        - ny * scale.xy[0] * y * scale.derivative[1]
                };
                let lateral = if options.cartoon && scale.derivative != [0.0; 2] {
                    normalize_or(lateral, ring.e1)
                } else {
                    lateral
                };
                let normal = normalize_or(
                    lateral
                        + ring
                            .tangent
                            .scale(derivative / ring.tangent.dot3d(ring.tangent).max(1.0e-12))
                        + ring.normal_offset,
                    ring.e1,
                );
                let mut color = color;
                if ring.ss == Secondary::Helix
                    && vertex.face
                        == if ring.flipped {
                            Face::Front
                        } else {
                            Face::Back
                        }
                {
                    if let Some(back) = options.helix_back_color {
                        color = back;
                    }
                }
                if ring.ss == Secondary::Sheet && matches!(vertex.face, Face::Side1 | Face::Side2) {
                    if let Some(side) = options.sheet_side_color {
                        color = side;
                    }
                }
                mesh.vertices.push(MeshVertex {
                    pick_atom_index,
                    position: ring.center + offset,
                    normal,
                    color,
                });
            }
            if let Some(previous) = last {
                let (previous, current) = if ri > 0
                    && (ring.center - strip.rings[ri - 1].center).length3d() < 1.0e-8
                {
                    let normal = normalize_or(ring.tangent, ring.e1).scale(-1.0);
                    let flat = mesh.vertices.len();
                    for offset in [previous, base] {
                        for j in 0..radial {
                            let mut vertex = mesh.vertices[offset + j].clone();
                            vertex.normal = normal;
                            mesh.vertices.push(vertex);
                        }
                    }
                    (flat, flat + radial)
                } else if !options.smooth_color
                    && (0..radial)
                        .any(|j| mesh.vertices[previous + j].color != mesh.vertices[base + j].color)
                {
                    let recolored = mesh.vertices.len();
                    for j in 0..radial {
                        let mut vertex = mesh.vertices[previous + j].clone();
                        vertex.color = mesh.vertices[base + j].color;
                        mesh.vertices.push(vertex);
                    }
                    (recolored, base)
                } else {
                    (previous, base)
                };
                for j in 0..radial {
                    let next = (j + 1) % radial;
                    let [a, b, c, d] = [previous + j, previous + next, current + j, current + next]
                        .map(|v| v as u32);
                    mesh.indices.extend_from_slice(&[a, d, c, a, b, d]);
                }
            }
            if ri == 0 || ri + 1 == strip.rings.len() {
                let cap = if ri == 0 {
                    strip.start_cap
                } else {
                    strip.end_cap
                };
                if cap != TubeCapType::None {
                    let next_color = strip.rings.get(1).map(&color_at).unwrap_or(color);
                    let cap_owner = if ri == 0 && strip.start_cap_next_color {
                        strip.rings.get(1).and_then(&owner_at).or(pick_atom_index)
                    } else {
                        pick_atom_index
                    };
                    let cap_color = if ri == 0 && strip.start_cap_next_color {
                        next_color
                    } else {
                        color
                    };
                    let mut cap_ring = *ring;
                    if ri == 0 && strip.unscaled_start_cap {
                        cap_ring.scale.xy = [1.0; 2];
                    }
                    if ri == 0 && strip.extra_start_cap {
                        cap_mesh(
                            &mut mesh,
                            base,
                            radial,
                            ring,
                            &strip.profile,
                            next_color,
                            strip.rings.get(1).and_then(&owner_at).or(pick_atom_index),
                            cap,
                            true,
                            putty,
                        );
                    }
                    cap_mesh(
                        &mut mesh,
                        base,
                        radial,
                        &cap_ring,
                        &strip.profile,
                        cap_color,
                        cap_owner,
                        cap,
                        ri == 0,
                        putty,
                    );
                }
            }
            last = Some(base);
        }
    }
    mesh
}

fn cap_mesh(
    mesh: &mut RenderMesh,
    _base: usize,
    radial: usize,
    ring: &Ring,
    profile: &Profile,
    color: [f32; 4],
    pick_atom_index: Option<u32>,
    cap: TubeCapType,
    start: bool,
    putty: f64,
) {
    let sign = if start { -1.0 } else { 1.0 };
    let normal = normalize_or(ring.tangent, Vector4::new3(0.0, 0.0, 1.0)).scale(sign);
    let e1 = ring.e1.scale(ring.scale.xy[0] * putty);
    let e2 = ring.e2.scale(ring.scale.xy[1] * putty);
    let offset = |i: usize| {
        let v = profile.vertices[i % radial];
        e1.scale(v.xy[0]) + e2.scale(v.xy[1])
    };
    if cap == TubeCapType::Flat {
        let mut fan = |center: Vector4, mut edge: Vec<Vector4>| {
            if start {
                edge.reverse();
            }
            let base = mesh.vertices.len() as u32;
            mesh.vertices.push(MeshVertex {
                pick_atom_index,
                position: center,
                normal,
                color,
            });
            for position in edge {
                mesh.vertices.push(MeshVertex {
                    pick_atom_index,
                    position,
                    normal,
                    color,
                });
            }
            for i in 1..mesh.vertices.len() as u32 - base - 1 {
                mesh.indices
                    .extend_from_slice(&[base, base + i, base + i + 1]);
            }
        };
        if let Some((nx, ny)) = profile.fancy_divisions {
            let first = ring.center + (offset(0) + offset(nx)).scale(0.5);
            let second = ring.center + (offset(nx + ny + 2) + offset(2 * nx + ny + 2)).scale(0.5);
            fan(first, (0..=nx).map(|i| ring.center + offset(i)).collect());
            fan(
                second,
                (0..=nx)
                    .map(|i| ring.center + offset(i + nx + ny + 2))
                    .collect(),
            );
            let mut edge = vec![first];
            edge.extend((0..=ny).map(|i| ring.center + offset(i + nx + 1)));
            edge.push(second);
            edge.extend((0..=ny).map(|i| ring.center + offset(i + 2 * nx + ny + 3)));
            edge.push(first);
            fan(ring.center, edge);
        } else {
            fan(
                ring.center,
                (0..=radial).map(|i| ring.center + offset(i)).collect(),
            );
        }
    } else {
        let radius = offset(0).length3d();
        let base = mesh.vertices.len();
        for i in 0..=5 {
            let t = i as f64 / 5.0;
            let g = (1.0 - t * t).sqrt();
            let center = ring.center + normal.scale(radius * t);
            let a = e1.scale(g);
            let b = e2.scale(g);
            for vertex in profile.vertices.iter() {
                let [x, y] = vertex.xy;
                let [nx, ny] = vertex.normal;
                let position = a.scale(x) + b.scale(y);
                let lateral = a.scale(nx) + b.scale(ny);
                let around = a.scale(-ny) + b.scale(nx);
                let normal = if i == 5 {
                    normal
                } else {
                    normalize_or(
                        lateral.scale(radius * g) - around.cross(position).scale(t * sign),
                        normal,
                    )
                };
                mesh.vertices.push(MeshVertex {
                    pick_atom_index,
                    position: center + position,
                    normal,
                    color,
                });
            }
            if i > 0 {
                let previous = base + (i - 1) * radial;
                let current = base + i * radial;
                for j in 0..radial {
                    let next = (j + 1) % radial;
                    let [a, b, c, d] = [previous + j, previous + next, current + j, current + next];
                    let indices = if start {
                        [a, c, b, b, c, d]
                    } else {
                        [a, d, c, a, b, d]
                    };
                    mesh.indices.extend(indices.map(|v| v as u32));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
