use super::{smoothing::SmoothSpline, *};
use std::collections::BTreeSet;

struct Element {
    begin: usize,
    end: usize,
    source_start: usize,
    source: Vec<MainchainPoint>,
    axis: SmoothSpline,
    normal: Option<SmoothSpline>,
    normal_average: Vector4,
    radius: f64,
    width: Option<SmoothSpline>,
}

fn ranges(
    points: &[MainchainPoint],
    split_helix: bool,
    include: impl Fn(&MainchainPoint) -> bool,
) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut start = None;
    for (i, point) in points.iter().enumerate() {
        if include(point) {
            if split_helix
                && start.is_some()
                && point.ss == Secondary::Helix
                && point.secondary_marker == b's'
            {
                result.push((start.take().unwrap(), i - 1));
            }
            start.get_or_insert(i);
        } else if let Some(start) = start.take() {
            result.push((start, i - 1));
        }
    }
    if let Some(start) = start {
        result.push((start, points.len() - 1));
    }
    result
}

fn weight(
    point: &MainchainPoint,
    options: &MoleculeRenderOptions,
    anchors: Option<&BTreeSet<usize>>,
) -> f64 {
    if anchors.is_some_and(|set| set.contains(&point.atom_index)) {
        options.cartoon_anchor_weight
    } else {
        1.0
    }
}

impl Element {
    fn new(
        points: &[MainchainPoint],
        begin: usize,
        end: usize,
        ss: Secondary,
        options: &MoleculeRenderOptions,
        anchors: Option<&BTreeSet<usize>>,
    ) -> Option<Self> {
        let source_start = begin.saturating_sub(1);
        let source_end = (end + 1).min(points.len() - 1);
        let source = points[source_start..=source_end].to_vec();
        let positions = source.iter().map(|p| p.pos).collect::<Vec<_>>();
        let weights = source
            .iter()
            .map(|p| weight(p, options, anchors))
            .collect::<Vec<_>>();
        let axis = SmoothSpline::new(
            &positions,
            (ss == Secondary::Sheet).then_some(weights.as_slice()),
            options.ribbon_smoothing.for_ss(ss),
            0.0,
            4,
        )?;
        let mut element = Self {
            begin,
            end,
            source_start,
            source,
            axis,
            normal: None,
            normal_average: Vector4::new3(1.0, 0.0, 0.0),
            radius: 1.0,
            width: None,
        };
        if ss == Secondary::Helix {
            let widths = positions
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    Vector4::new3(
                        (*p - element.axis.interpolate(i as f64).position).length3d(),
                        0.0,
                        0.0,
                    )
                })
                .collect::<Vec<_>>();
            element.radius = widths.iter().map(|v| v.x).sum::<f64>() / widths.len() as f64;
            if widths.len() >= 3 {
                element.width = SmoothSpline::new(
                    &widths,
                    None,
                    options.cartoon_helix_width_smoothing,
                    0.0,
                    10,
                );
            }
        } else {
            let first = 1 + usize::from(begin > 0);
            let last = positions.len() - 1 - usize::from(end + 1 < points.len());
            if first <= last && last < positions.len() - 1 {
                let mut normals = Vec::new();
                let mut previous = Vector4::new3(0.0, 0.0, 0.0);
                let mut average = previous;
                for i in first..=last {
                    let mut normal = pivot_binormal(&element.source, i);
                    if normal.dot3d(previous) < 0.0 {
                        normal = normal.scale(-1.0);
                    }
                    normals.push(normal);
                    average = average + normal;
                    previous = normal;
                }
                element.normal_average = normalize_or(average, Vector4::new3(1.0, 0.0, 0.0));
                element.normal = SmoothSpline::new(
                    &normals,
                    None,
                    options.cartoon_sheet_normal_smoothing,
                    0.0,
                    4,
                );
            }
        }
        Some(element)
    }

    fn radius(&self, t: f64, options: &MoleculeRenderOptions) -> (f64, f64) {
        let (radius, derivative) = match options.cartoon_helix_width_mode {
            CartoonHelixWidthMode::Constant => (options.cartoon_helix_width, 0.0),
            CartoonHelixWidthMode::Wavy if self.width.is_some() => {
                let value = self.width.as_ref().unwrap().interpolate(t);
                (value.position.x, value.tangent.x)
            }
            _ => (self.radius, 0.0),
        };
        (
            (radius + options.cartoon_helix_width_plus).max(1.0e-8),
            derivative,
        )
    }

    fn helix_junction(
        &self,
        point: MainchainPoint,
        index: usize,
        coil: &Profile,
        options: &MoleculeRenderOptions,
    ) -> Vector4 {
        let t = (index - self.source_start) as f64;
        let center = self.axis.interpolate(t).position;
        let limit = (self.radius(t, options).0 - coil.size[0]).max(0.0);
        let vector = point.pos - center;
        let length = vector.length3d();
        if length <= limit {
            point.pos
        } else {
            center + vector.scale(limit / length)
        }
    }

    fn color_parameter(&self, t: f64) -> f64 {
        (t + self.source_start as f64).clamp(self.begin as f64, self.end as f64)
    }

    fn sheet_ring(&self, t: f64, scale: Scale) -> Ring {
        let axis = self.axis.interpolate(t);
        let normal = self
            .normal
            .as_ref()
            .map(|s| {
                s.interpolate(t - 1.0 - u8::from(self.begin > 0) as f64)
                    .position
            })
            .unwrap_or(self.normal_average);
        let tangent = normalize_or(axis.tangent, Vector4::new3(0.0, 0.0, 1.0));
        let e1 = normalize_or(normal.cross(tangent), perpendicular(tangent));
        let e2 = tangent.cross(e1);
        Ring {
            center: axis.position,
            tangent: axis.tangent,
            e1,
            e2,
            scale,
            par: self.color_parameter(t),
            ss: Secondary::Sheet,
            flipped: false,
            normal_offset: Vector4::new3(0.0, 0.0, 0.0),
        }
    }
}

pub(crate) fn strips(
    points: &[MainchainPoint],
    options: &MoleculeRenderOptions,
    anchors: Option<&BTreeSet<usize>>,
) -> Vec<Strip> {
    let mut result = Vec::new();
    let detail = options.ribbon_axial_detail.unwrap_or(8).max(2) as usize;
    let coil = Profile::for_section(Secondary::Coil, options);
    let mut helices = Vec::new();
    let mut helix_index = vec![None; points.len()];
    if !options.cartoon_helix_ribbon {
        for (begin, end) in ranges(points, true, |point| point.ss == Secondary::Helix) {
            let Some(element) =
                Element::new(points, begin, end, Secondary::Helix, options, anchors)
            else {
                continue;
            };
            let t0 = if begin > 0 {
                1.0 - options.cartoon_helix_extend
            } else {
                0.0
            };
            let tn = (element.source.len() - 1) as f64
                - if end + 1 < points.len() {
                    1.0 - options.cartoon_helix_extend
                } else {
                    0.0
                };
            let count = ((tn - t0) * detail as f64).floor() as usize;
            if count == 0 {
                continue;
            }
            let mut previous = Vector4::new3(1.0, 0.0, 0.0);
            let mut rings = Vec::with_capacity(count + 1);
            for j in 0..=count {
                let t = t0 + j as f64 * (tn - t0) / count as f64;
                let axis = element.axis.interpolate(t);
                if j == 0 {
                    previous = axis.position - points[begin].pos;
                }
                let (radius, derivative) = element.radius(t, options);
                let tangent = normalize_or(axis.tangent, Vector4::new3(0.0, 0.0, 1.0));
                let e1 =
                    normalize_or(previous.cross(tangent), perpendicular(tangent)).scale(radius);
                let e2 = normalize_or(tangent.cross(e1), previous).scale(radius);
                rings.push(Ring {
                    center: axis.position,
                    tangent: axis.tangent,
                    e1,
                    e2,
                    scale: unit_scale(t),
                    par: element.color_parameter(t),
                    ss: Secondary::Helix,
                    flipped: false,
                    normal_offset: axis.tangent.scale(-derivative),
                });
                previous = e2;
            }
            result.push(Strip {
                unscaled_start_cap: false,
                extra_start_cap: false,
                start_cap_next_color: false,
                rings,
                profile: Profile::for_section(Secondary::Helix, options),
                start_cap: options.start_cap_type,
                end_cap: options.end_cap_type,
            });
            helix_index[begin..=end].fill(Some(helices.len()));
            helices.push(element);
        }
    }
    let mut sheets = Vec::new();
    let mut sheet_index = vec![None; points.len()];
    for (begin, end) in ranges(points, false, |point| point.ss == Secondary::Sheet) {
        let Some(element) = Element::new(points, begin, end, Secondary::Sheet, options, anchors)
        else {
            continue;
        };
        let t0 = if begin > 0 { 0.5 } else { 0.0 };
        let tn = (element.source.len() - 1) as f64 - if end + 1 < points.len() { 0.5 } else { 0.0 };
        let count = ((tn - 1.0 - t0) * detail as f64).floor().max(0.0) as usize;
        if count > 0 {
            let profile = Profile::for_section(Secondary::Sheet, options);
            let rings = (0..=count)
                .map(|j| {
                    let t = t0 + j as f64 * (tn - 1.0 - t0) / count as f64;
                    element.sheet_ring(t, unit_scale(t))
                })
                .collect();
            result.push(Strip {
                unscaled_start_cap: false,
                extra_start_cap: false,
                start_cap_next_color: false,
                rings,
                profile: profile.clone(),
                start_cap: options.start_cap_type,
                end_cap: TubeCapType::None,
            });
            let scales = junction(&profile, &coil, true, detail + 1, options.sheet_head);
            let rings = scales
                .into_iter()
                .map(|scale| element.sheet_ring(tn - 1.0 + scale.t, scale))
                .collect();
            result.push(Strip {
                unscaled_start_cap: false,
                extra_start_cap: false,
                start_cap_next_color: false,
                rings,
                profile,
                start_cap: TubeCapType::None,
                end_cap: options.end_cap_type,
            });
        }
        sheet_index[begin..=end].fill(Some(sheets.len()));
        sheets.push(element);
    }
    let coil_ranges = ranges(points, false, |p| {
        p.ss == Secondary::Coil || (options.cartoon_helix_ribbon && p.ss == Secondary::Helix)
    });
    for (begin, end) in coil_ranges {
        let mut source = Vec::new();
        let mut weights = Vec::new();
        let mut start_id = 0.0;
        let delta = begin.saturating_sub(1);
        let anchor_weight = options.cartoon_anchor_weight;
        if begin > 0 {
            if let Some(index) = sheet_index[begin - 1] {
                let sheet = &sheets[index];
                let tend = (sheet.source.len() - 1) as f64;
                for t in [tend - 1.5, tend - 0.5] {
                    let mut point = points[begin - 1];
                    point.pos = sheet.axis.interpolate(t).position;
                    point.carbonyl = None;
                    source.push(point);
                    weights.push(anchor_weight);
                }
                start_id = -1.0;
            } else {
                let mut point = points[begin - 1];
                if let Some(index) = helix_index[begin - 1] {
                    point.pos = helices[index].helix_junction(point, begin - 1, &coil, options);
                }
                source.push(point);
                weights.push(anchor_weight);
            }
        }
        for i in begin..=end {
            source.push(points[i]);
            let w = if options.cartoon_helix_ribbon && points[i].ss == Secondary::Helix {
                10.0
            } else if i == begin && begin > 0 && helix_index[begin - 1].is_some() {
                anchor_weight
            } else {
                weight(&points[i], options, anchors)
            };
            weights.push(w);
        }
        let real_size;
        if end + 1 < points.len() {
            if let Some(index) = sheet_index[end + 1] {
                let sheet = &sheets[index];
                let mut point = points[end + 1];
                point.pos = sheet.axis.interpolate(0.5).position;
                point.carbonyl = None;
                source.push(point);
                weights.push(anchor_weight);
                real_size = source.len() as f64 + start_id;
                point.pos = sheet.axis.interpolate(1.5).position;
                source.push(point);
                weights.push(anchor_weight);
            } else {
                if let Some(weight) = weights.last_mut() {
                    *weight = anchor_weight;
                }
                let mut point = points[end + 1];
                if let Some(index) = helix_index[end + 1] {
                    point.pos = helices[index].helix_junction(point, end + 1, &coil, options);
                }
                source.push(point);
                weights.push(anchor_weight);
                real_size = source.len() as f64 + start_id;
            }
        } else {
            if let Some(weight) = weights.last_mut() {
                *weight = anchor_weight;
            }
            real_size = source.len() as f64 + start_id;
        }
        if real_size < 2.0 {
            continue;
        }
        let positions = source.iter().map(|p| p.pos).collect::<Vec<_>>();
        let Some(axis) = SmoothSpline::new(
            &positions,
            Some(&weights),
            options.ribbon_smoothing.coil,
            start_id,
            4,
        ) else {
            continue;
        };
        let count = ((real_size - 1.0) * detail as f64) as usize;
        let color_par = |t: f64| {
            t.clamp(
                u8::from(begin > 0) as f64,
                real_size - 1.0 - u8::from(end + 1 < points.len()) as f64,
            ) + delta as f64
        };
        if !options.cartoon_helix_ribbon {
            let mut previous = Vector4::new3(1.0, 0.0, 0.0);
            let rings = (0..=count)
                .map(|j| {
                    let t = j as f64 / detail as f64;
                    let sample = axis.interpolate(t);
                    let tangent = normalize_or(sample.tangent, Vector4::new3(0.0, 0.0, 1.0));
                    let e1 = normalize_or(previous.cross(tangent), perpendicular(tangent));
                    let e2 = normalize_or(tangent.cross(e1), previous);
                    previous = e2;
                    Ring {
                        center: sample.position,
                        tangent: sample.tangent,
                        e1,
                        e2,
                        scale: unit_scale(t),
                        par: color_par(t),
                        ss: Secondary::Coil,
                        flipped: false,
                        normal_offset: Vector4::new3(0.0, 0.0, 0.0),
                    }
                })
                .collect();
            result.push(Strip {
                unscaled_start_cap: false,
                extra_start_cap: false,
                start_cap_next_color: false,
                rings,
                profile: coil.clone(),
                start_cap: options.start_cap_type,
                end_cap: options.end_cap_type,
            });
        } else {
            let mut previous = None;
            let normals = (0..source.len())
                .map(|i| {
                    let sample = axis.interpolate(i as f64 + start_id);
                    let (normal, _) =
                        orient_binormal(pivot_binormal(&source, i), sample.tangent, previous);
                    previous = Some((normal, sample.tangent));
                    sample.position + normal
                })
                .collect::<Vec<_>>();
            let Some(normal) = SmoothSpline::new(&normals, None, -10.0, start_id, 4) else {
                continue;
            };
            let mut groups = Vec::new();
            let mut group_start = 0;
            let mut previous_key = (Secondary::Coil, 0);
            for j in 0..=count {
                let index =
                    (color_par(j as f64 / detail as f64).floor() as usize).min(points.len() - 1);
                let point = points[index];
                let key = if point.ss == Secondary::Helix {
                    (Secondary::Helix, point.secondary_marker)
                } else {
                    (Secondary::Coil, 0)
                };
                if j > 0 && key != previous_key {
                    groups.push((group_start, j, previous_key));
                    group_start = j;
                }
                previous_key = key;
            }
            groups.push((group_start, count, previous_key));
            let last = groups.len() - 1;
            for (i, (start, end, _)) in groups.iter_mut().enumerate() {
                if i > 0 {
                    *start = start.saturating_sub(detail / 2);
                }
                if i < last {
                    *end = end.saturating_sub(detail / 2);
                }
            }
            for i in 0..groups.len() {
                if groups[i].2 .1 == b's' && groups[i].1 - groups[i].0 > detail {
                    groups[i].1 = groups[i].0 + detail;
                    if i < last {
                        groups[i + 1].0 = groups[i].1;
                    }
                }
            }
            for (start, end, (ss, marker)) in groups {
                let profile = Profile::for_section(ss, options);
                let transition = ss == Secondary::Helix && matches!(marker, b's' | b'e');
                let scales = if transition {
                    if marker == b'e' {
                        junction(&profile, &coil, true, detail + 1, options.helix_head)
                    } else {
                        junction(&coil, &profile, false, detail + 1, options.helix_tail)
                    }
                } else {
                    (start..=end)
                        .map(|j| unit_scale((j - start) as f64 / detail as f64))
                        .collect()
                };
                let rings = scales
                    .into_iter()
                    .map(|scale| {
                        let t = start as f64 / detail as f64 + scale.t;
                        let sample = axis.interpolate(t);
                        let tangent = normalize_or(sample.tangent, Vector4::new3(0.0, 0.0, 1.0));
                        let e2 = normalize_or(
                            normal.interpolate(t).position - sample.position,
                            perpendicular(tangent),
                        );
                        let e1 = e2.cross(tangent);
                        Ring {
                            center: sample.position,
                            tangent: sample.tangent,
                            e1,
                            e2,
                            scale,
                            par: color_par(t),
                            ss,
                            flipped: false,
                            normal_offset: Vector4::new3(0.0, 0.0, 0.0),
                        }
                    })
                    .collect();
                result.push(Strip {
                    unscaled_start_cap: false,
                    extra_start_cap: false,
                    start_cap_next_color: false,
                    rings,
                    profile,
                    start_cap: if ss == Secondary::Coil || marker == b's' {
                        options.start_cap_type
                    } else {
                        TubeCapType::None
                    },
                    end_cap: if ss == Secondary::Coil || marker == b'e' {
                        options.end_cap_type
                    } else {
                        TubeCapType::None
                    },
                });
            }
        }
    }
    result
}

fn unit_scale(t: f64) -> Scale {
    Scale {
        t,
        xy: [1.0; 2],
        derivative: [0.0; 2],
    }
}
