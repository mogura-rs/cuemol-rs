//! Pipeline optimization (port of LittleCMS 2.19.1 cmsopt.c, MIT).

use super::base::*;
use super::gamma::ToneCurve;
use super::lut::*;

const PRELINEARIZATION_POINTS: u32 = 4096;

fn remove1op(lut: &mut Pipeline, op: u32) -> bool {
    let before = lut.stages.len();
    lut.stages.retain(|s| s.implements != op);
    lut.stages.len() != before
}

fn remove2op(lut: &mut Pipeline, op1: u32, op2: u32) -> bool {
    let mut any = false;
    let mut i = 0;
    while i < lut.stages.len() {
        if i + 1 >= lut.stages.len() {
            return any;
        }
        if lut.stages[i].implements == op1 && lut.stages[i + 1].implements == op2 {
            lut.stages.remove(i + 1);
            lut.stages.remove(i);
            any = true;
        } else {
            i += 1;
        }
    }
    any
}

fn is_float_matrix_identity(a: &Mat3) -> bool {
    let id = mat3_identity();
    for i in 0..3 {
        for j in 0..3 {
            if (id[i][j] - a[i][j]).abs() >= 0.00001f32 as f64 {
                return false;
            }
        }
    }
    true
}

fn multiply_matrix(lut: &mut Pipeline) -> bool {
    let mut any = false;
    let mut i = 0;
    while i < lut.stages.len() {
        if i + 1 >= lut.stages.len() {
            return any;
        }
        let (s1, s2) = (&lut.stages[i], &lut.stages[i + 1]);
        if s1.implements == SIG_MATRIX_ELEM && s2.implements == SIG_MATRIX_ELEM {
            let (StageData::Matrix { m: m1, off: o1 }, StageData::Matrix { m: m2, off: o2 }) =
                (&s1.data, &s2.data)
            else {
                return false;
            };
            if o1.is_some()
                || o2.is_some()
                || s1.in_ch != 3
                || s1.out_ch != 3
                || s2.in_ch != 3
                || s2.out_ch != 3
            {
                return false;
            }
            let res = mat3_per(&mat3_from_flat(m2), &mat3_from_flat(m1));
            lut.stages.remove(i + 1);
            lut.stages.remove(i);
            if !is_float_matrix_identity(&res) {
                let Some(mult) = Stage::matrix(3, 3, &mat3_flat(&res), None) else {
                    return false;
                };
                lut.stages.insert(i, mult);
            }
            any = true;
        } else {
            i += 1;
        }
    }
    any
}

fn pre_optimize(lut: &mut Pipeline) -> bool {
    let mut any = false;
    loop {
        let mut opt = false;
        opt |= remove1op(lut, SIG_IDENTITY_ELEM);
        opt |= remove2op(lut, SIG_XYZ2LAB_ELEM, SIG_LAB2XYZ_ELEM);
        opt |= remove2op(lut, SIG_LAB2XYZ_ELEM, SIG_XYZ2LAB_ELEM);
        opt |= remove2op(lut, SIG_LABV4_TO_V2, SIG_LABV2_TO_V4);
        opt |= remove2op(lut, SIG_LABV2_TO_V4, SIG_LABV4_TO_V2);
        opt |= remove2op(lut, SIG_LAB2_FLOAT_PCS, SIG_FLOAT_PCS2_LAB);
        opt |= remove2op(lut, SIG_XYZ2_FLOAT_PCS, SIG_FLOAT_PCS2_XYZ);
        opt |= multiply_matrix(lut);
        if opt {
            any = true;
        } else {
            break;
        }
    }
    any
}

fn xform_sampler16(lut: &Pipeline, input: &[u16], out: &mut [u16]) -> bool {
    let mut fin = [0f32; MAX_CHANNELS];
    let mut fout = [0f32; MAX_STAGE_CHANNELS];
    for i in 0..lut.input_channels {
        fin[i] = (input[i] as f64 / 65535.0) as f32;
    }
    lut.eval_float(&fin, &mut fout);
    for i in 0..lut.output_channels {
        out[i] = quick_saturate_word(fout[i] as f64 * 65535.0);
    }
    true
}

fn all_curves_are_linear(stage: &Stage) -> bool {
    match stage.curves() {
        Some(c) => c[..stage.out_ch].iter().all(|c| c.is_linear()),
        None => false,
    }
}

fn patch_lut(clut: &mut Stage, at: &[u16], value: &[u16], n_out: usize, n_in: usize) -> bool {
    let StageData::Clut16 { params, table } = &mut clut.data else {
        return false;
    };
    let p = |k: usize| (at[k] as f64 * params.domain[k] as f64) / 65535.0;
    let index: i32 = match n_in {
        4 => {
            let (px, py, pz, pw) = (p(0), p(1), p(2), p(3));
            let (x0, y0, z0, w0) = (
                px.floor() as i32,
                py.floor() as i32,
                pz.floor() as i32,
                pw.floor() as i32,
            );
            if px - x0 as f64 != 0.0
                || py - y0 as f64 != 0.0
                || pz - z0 as f64 != 0.0
                || pw - w0 as f64 != 0.0
            {
                return false;
            }
            params.opta[3] as i32 * x0
                + params.opta[2] as i32 * y0
                + params.opta[1] as i32 * z0
                + params.opta[0] as i32 * w0
        }
        3 => {
            let (px, py, pz) = (p(0), p(1), p(2));
            let (x0, y0, z0) = (px.floor() as i32, py.floor() as i32, pz.floor() as i32);
            if px - x0 as f64 != 0.0 || py - y0 as f64 != 0.0 || pz - z0 as f64 != 0.0 {
                return false;
            }
            params.opta[2] as i32 * x0 + params.opta[1] as i32 * y0 + params.opta[0] as i32 * z0
        }
        1 => {
            let px = p(0);
            let x0 = px.floor() as i32;
            if px - x0 as f64 != 0.0 {
                return false;
            }
            params.opta[0] as i32 * x0
        }
        _ => return false,
    };
    for i in 0..n_out {
        if let Some(t) = table.get_mut(index as usize + i) {
            *t = value[i];
        }
    }
    true
}

fn whites_are_equal(n: usize, w1: &[u16], w2: &[u16]) -> bool {
    for i in 0..n {
        if (w1[i] as i32 - w2[i] as i32).abs() > 0xf000 {
            return true;
        }
        if w1[i] != w2[i] {
            return false;
        }
    }
    true
}

fn fix_white_misalignment(lut: &mut Pipeline, entry: u32, exit: u32) -> bool {
    let Some((white_in, _)) = end_points_by_space(entry) else {
        return false;
    };
    let Some((white_out, _)) = end_points_by_space(exit) else {
        return false;
    };
    let (n_ins, n_outs) = (white_in.len(), white_out.len());
    if lut.input_channels != n_ins || lut.output_channels != n_outs {
        return false;
    }
    let mut win16 = [0u16; MAX_CHANNELS];
    win16[..n_ins].copy_from_slice(white_in);
    let mut obtained = [0u16; MAX_CHANNELS];
    lut.eval16(&win16, &mut obtained);
    if whites_are_equal(n_outs, white_out, &obtained) {
        return true;
    }
    use SIG_CLUT_ELEM as C;
    use SIG_CURVE_SET_ELEM as V;
    let (pre, clut, post) = if lut.check_stages(&[V, C, V]) {
        (Some(0), 1, Some(2))
    } else if lut.check_stages(&[V, C]) {
        (Some(0), 1, None)
    } else if lut.check_stages(&[C, V]) {
        (None, 0, Some(1))
    } else if lut.check_stages(&[C]) {
        (None, 0, None)
    } else {
        return false;
    };
    let mut white_in_t = [0u16; MAX_CHANNELS];
    for i in 0..n_ins {
        white_in_t[i] = match pre {
            Some(s) => lut.stages[s]
                .curves()
                .map_or(white_in[i], |c| c[i].eval16(white_in[i])),
            None => white_in[i],
        };
    }
    let mut white_out_t = [0u16; MAX_CHANNELS];
    for i in 0..n_outs {
        white_out_t[i] = match post {
            Some(s) => match lut.stages[s].curves().and_then(|c| c[i].reverse()) {
                Some(inv) => inv.eval16(white_out[i]),
                None => white_out[i],
            },
            None => white_out[i],
        };
    }
    patch_lut(
        &mut lut.stages[clut],
        &white_in_t,
        &white_out_t,
        n_outs,
        n_ins,
    );
    true
}

fn optimize_by_resampling(
    lut: &mut Pipeline,
    intent: u32,
    input: &mut u32,
    output: &mut u32,
    flags: &mut u32,
) -> bool {
    if formatter_is_float(*input) || formatter_is_float(*output) {
        return false;
    }
    let cs = icc_color_space(t_colorspace(*input));
    let out_cs = icc_color_space(t_colorspace(*output));
    if cs == 0 || out_cs == 0 {
        return false;
    }
    let n_grid = if lut.stages.is_empty() {
        2
    } else {
        if *flags & FLAGS_FORCE_CLUT == 0 && cs == SIG_LAB_DATA && t_bytes(*input) == 2 {
            return false;
        }
        reasonable_gridpoints(cs, *flags)
    };
    let mut src = lut.clone();
    let Some(mut dest) = Pipeline::new(src.input_channels, src.output_channels) else {
        return false;
    };
    let mut new_prelin = None;
    if *flags & FLAGS_CLUT_PRE_LINEARIZATION != 0 {
        if let Some(first) = src.stages.first() {
            if first.typ == SIG_CURVE_SET_ELEM && !all_curves_are_linear(first) {
                let dup = first.clone();
                if !dest.insert_begin(Some(dup.clone())) {
                    return false;
                }
                new_prelin = dup.curves().map(|c| c.to_vec());
                src.stages.remove(0);
                src.bless();
            }
        }
    }
    let Some(clut) = Stage::clut16(n_grid, src.input_channels, src.output_channels, None) else {
        return false;
    };
    if !dest.insert_end(Some(clut)) {
        return false;
    }
    let clut_index = dest.stages.len() - 1;
    let mut new_postlin = None;
    if *flags & FLAGS_CLUT_POST_LINEARIZATION != 0 {
        if let Some(last) = src.stages.last() {
            if last.typ == SIG_CURVE_SET_ELEM && !all_curves_are_linear(last) {
                let dup = last.clone();
                if !dest.insert_end(Some(dup.clone())) {
                    return false;
                }
                new_postlin = dup.curves().map(|c| c.to_vec());
                src.stages.pop();
                src.bless();
            }
        }
    }
    let sampled = dest.stages[clut_index].sample_clut16(|i, o| xform_sampler16(&src, i, o), false);
    if !sampled {
        return false;
    }
    if new_prelin.is_none() && new_postlin.is_none() {
        dest.opt = Some(Opt16::Clut(clut_index));
    } else {
        dest.opt = Some(Opt16::Prelin16(Box::new(Prelin16 {
            n_inputs: dest.input_channels,
            n_outputs: dest.output_channels,
            curves_in: new_prelin,
            clut_stage: clut_index,
            curves_out: new_postlin,
        })));
    }
    if intent == INTENT_ABSOLUTE_COLORIMETRIC {
        *flags |= FLAGS_NOWHITEONWHITEFIXUP;
    }
    if *flags & FLAGS_NOWHITEONWHITEFIXUP == 0 {
        fix_white_misalignment(&mut dest, cs, out_cs);
    }
    *lut = dest;
    true
}

fn slope_limiting(g: &mut ToneCurve) {
    let n = g.n_entries() as i32;
    let at_begin = (n as f64 * 0.02 + 0.5).floor() as i32;
    let at_end = n - at_begin - 1;
    let (begin_val, end_val) = if g.is_descending() {
        (0xffff, 0)
    } else {
        (0, 0xffff)
    };
    let t = &mut g.table16;
    let val = t[at_begin as usize] as f64;
    let slope = (val - begin_val as f64) / at_begin as f64;
    let beta = val - slope * at_begin as f64;
    for i in 0..at_begin {
        t[i as usize] = quick_saturate_word(i as f64 * slope + beta);
    }
    let val = t[at_end as usize] as f64;
    let slope = (end_val as f64 - val) / at_begin as f64;
    let beta = val - slope * at_end as f64;
    for i in at_end..n {
        t[i as usize] = quick_saturate_word(i as f64 * slope + beta);
    }
}

fn prelin_opt8_alloc(
    params: &super::intrp::InterpParams,
    g: &[ToneCurve],
    clut_stage: usize,
) -> Prelin8 {
    let mut p8 = Prelin8 {
        clut_stage,
        rx: [0; 256],
        ry: [0; 256],
        rz: [0; 256],
        x0: [0; 256],
        y0: [0; 256],
        z0: [0; 256],
    };
    for i in 0..256 {
        let w = from_8_to_16(i as u8);
        let input = [g[0].eval16(w), g[1].eval16(w), g[2].eval16(w)];
        let v1 = to_fixed_domain((input[0] as u32).wrapping_mul(params.domain[0]) as i32);
        let v2 = to_fixed_domain((input[1] as u32).wrapping_mul(params.domain[1]) as i32);
        let v3 = to_fixed_domain((input[2] as u32).wrapping_mul(params.domain[2]) as i32);
        p8.x0[i] = params.opta[2].wrapping_mul((v1 >> 16) as u32);
        p8.y0[i] = params.opta[1].wrapping_mul((v2 >> 16) as u32);
        p8.z0[i] = params.opta[0].wrapping_mul((v3 >> 16) as u32);
        p8.rx[i] = (v1 & 0xFFFF) as u16;
        p8.ry[i] = (v2 & 0xFFFF) as u16;
        p8.rz[i] = (v3 & 0xFFFF) as u16;
    }
    p8
}

fn is_degenerated(g: &ToneCurve) -> bool {
    let n = g.n_entries();
    let zeros = g.table16.iter().filter(|&&v| v == 0).count() as u32;
    let poles = g.table16.iter().filter(|&&v| v == 0xffff).count() as u32;
    if zeros == 1 && poles == 1 {
        return false;
    }
    zeros > n / 20 || poles > n / 20
}

fn optimize_by_computing_linearization(
    lut: &mut Pipeline,
    intent: u32,
    input: &mut u32,
    output: &mut u32,
    flags: &mut u32,
) -> bool {
    if formatter_is_float(*input) || formatter_is_float(*output) {
        return false;
    }
    if t_colorspace(*input) != PT_RGB || (*input >> 12) & 1 != 0 {
        return false;
    }
    if t_colorspace(*output) != PT_RGB || (*output >> 12) & 1 != 0 {
        return false;
    }
    if !formatter_is_8bit(*input) && *flags & FLAGS_CLUT_PRE_LINEARIZATION == 0 {
        return false;
    }
    let original = &*lut;
    let cs = icc_color_space(t_colorspace(*input));
    let out_cs = icc_color_space(t_colorspace(*output));
    if cs == 0 || out_cs == 0 {
        return false;
    }
    let n_grid = reasonable_gridpoints(cs, *flags);
    let Some(last) = original.stages.last() else {
        return false;
    };
    if let Some(curves) = last.curves() {
        if curves.iter().any(is_degenerated) {
            return false;
        }
    }
    let n_in = original.input_channels;
    let mut trans = Vec::with_capacity(n_in);
    for _ in 0..n_in {
        let Some(c) = ToneCurve::tabulated16(PRELINEARIZATION_POINTS, None) else {
            return false;
        };
        trans.push(c);
    }
    let mut fin = [0f32; MAX_CHANNELS];
    let mut fout = [0f32; MAX_STAGE_CHANNELS];
    for i in 0..PRELINEARIZATION_POINTS as usize {
        let v = (i as f64 / (PRELINEARIZATION_POINTS - 1) as f64) as f32;
        fin[..n_in].fill(v);
        original.eval_float(&fin, &mut fout);
        for t in 0..n_in {
            trans[t].table16[i] = quick_saturate_word(fout[t] as f64 * 65535.0);
        }
    }
    for t in trans.iter_mut() {
        slope_limiting(t);
    }
    let mut suitable = true;
    for t in &trans {
        if !suitable {
            break;
        }
        if !t.is_monotonic() || is_degenerated(t) {
            suitable = false;
        }
    }
    if !suitable {
        return false;
    }
    let mut reverse = Vec::with_capacity(n_in);
    for t in &trans {
        let Some(r) = t.reverse_ex(PRELINEARIZATION_POINTS) else {
            return false;
        };
        reverse.push(r);
    }
    let mut plus_curves = original.clone();
    if !plus_curves.insert_begin(Stage::tone_curves(n_in, Some(&reverse))) {
        return false;
    }
    let Some(mut optimized) = Pipeline::new(n_in, original.output_channels) else {
        return false;
    };
    if !optimized.insert_begin(Stage::tone_curves(n_in, Some(&trans))) {
        return false;
    }
    if !optimized.insert_end(Stage::clut16(n_grid, n_in, original.output_channels, None)) {
        return false;
    }
    if !optimized.stages[1].sample_clut16(|i, o| xform_sampler16(&plus_curves, i, o), false) {
        return false;
    }
    let prelin_curves = optimized.stages[0]
        .curves()
        .map(|c| c.to_vec())
        .unwrap_or_default();
    if formatter_is_8bit(*input) {
        let StageData::Clut16 { params, .. } = &optimized.stages[1].data else {
            return false;
        };
        let p8 = prelin_opt8_alloc(params, &prelin_curves, 1);
        optimized.opt = Some(Opt16::Prelin8(Box::new(p8)));
    } else {
        optimized.opt = Some(Opt16::Prelin16(Box::new(Prelin16 {
            n_inputs: 3,
            n_outputs: 3,
            curves_in: Some(prelin_curves),
            clut_stage: 1,
            curves_out: None,
        })));
    }
    if intent == INTENT_ABSOLUTE_COLORIMETRIC {
        *flags |= FLAGS_NOWHITEONWHITEFIXUP;
    }
    if *flags & FLAGS_NOWHITEONWHITEFIXUP == 0
        && !fix_white_misalignment(&mut optimized, cs, out_cs)
    {
        return false;
    }
    *lut = optimized;
    true
}

fn curves_alloc(n_elements: usize, curves: &[ToneCurve]) -> Curves16Data {
    let tables = curves
        .iter()
        .map(|g| {
            (0..n_elements)
                .map(|j| {
                    if n_elements == 256 {
                        g.eval16(from_8_to_16(j as u8))
                    } else {
                        g.eval16(j as u16)
                    }
                })
                .collect()
        })
        .collect();
    Curves16Data { curves: tables }
}

fn optimize_by_joining_curves(
    lut: &mut Pipeline,
    input: &mut u32,
    output: &mut u32,
    flags: &mut u32,
) -> bool {
    if formatter_is_float(*input) || formatter_is_float(*output) {
        return false;
    }
    if lut.stages.iter().any(|s| s.typ != SIG_CURVE_SET_ELEM) {
        return false;
    }
    let src = &*lut;
    let Some(mut dest) = Pipeline::new(src.input_channels, src.output_channels) else {
        return false;
    };
    let n = src.input_channels;
    let mut gamma = Vec::with_capacity(n);
    for _ in 0..n {
        let Some(c) = ToneCurve::tabulated16(PRELINEARIZATION_POINTS, None) else {
            return false;
        };
        gamma.push(c);
    }
    let mut fin = [0f32; MAX_CHANNELS];
    let mut fout = [0f32; MAX_STAGE_CHANNELS];
    for i in 0..PRELINEARIZATION_POINTS as usize {
        fin[..n].fill((i as f64 / (PRELINEARIZATION_POINTS - 1) as f64) as f32);
        src.eval_float(&fin, &mut fout);
        for j in 0..n {
            gamma[j].table16[i] = quick_saturate_word(fout[j] as f64 * 65535.0);
        }
    }
    let Some(obtained) = Stage::tone_curves(n, Some(&gamma)) else {
        return false;
    };
    if !all_curves_are_linear(&obtained) {
        let curves = obtained.curves().map(|c| c.to_vec()).unwrap_or_default();
        if !dest.insert_begin(Some(obtained)) {
            return false;
        }
        *flags |= FLAGS_NOCACHE;
        dest.opt = Some(if formatter_is_8bit(*input) {
            Opt16::Curves8(Box::new(curves_alloc(256, &curves)))
        } else {
            Opt16::Curves16(Box::new(curves_alloc(65536, &curves)))
        });
    } else {
        if !dest.insert_begin(Some(Stage::identity(n))) {
            return false;
        }
        *flags |= FLAGS_NOCACHE;
        dest.opt = Some(Opt16::Identity);
    }
    *lut = dest;
    true
}

fn double_to_1fixed14(x: f64) -> i32 {
    (x * 16384.0 + 0.5).floor() as i32
}

fn fill_first_shaper(curve: &ToneCurve) -> [i32; 256] {
    let mut table = [0i32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let r = (i as f64 / 255.0) as f32;
        let y = curve.eval_float(r);
        *t = if (y as f64) < 131072.0 {
            double_to_1fixed14(y as f64)
        } else {
            0x7fffffff
        };
    }
    table
}

fn fill_second_shaper(curve: &ToneCurve, is_8bits: bool) -> Vec<u16> {
    (0..16385)
        .map(|i| {
            let r = (i as f64 / 16384.0) as f32;
            let mut val = curve.eval_float(r);
            if val < 0.0 {
                val = 0.0;
            }
            if val > 1.0 {
                val = 1.0;
            }
            if is_8bits {
                let w = quick_saturate_word(val as f64 * 65535.0);
                from_8_to_16(from_16_to_8(w))
            } else {
                quick_saturate_word(val as f64 * 65535.0)
            }
        })
        .collect()
}

fn optimize_matrix_shaper(
    lut: &mut Pipeline,
    intent: u32,
    input: &mut u32,
    output: &mut u32,
    flags: &mut u32,
) -> bool {
    if t_channels(*input) != 3 || t_channels(*output) != 3 {
        return false;
    }
    if !formatter_is_8bit(*input) {
        return false;
    }
    let src = &*lut;
    use SIG_CURVE_SET_ELEM as V;
    use SIG_MATRIX_ELEM as M;
    let (res, offset, identity, c1, c2) = if src.check_stages(&[V, M, M, V]) {
        let (m1, m2) = (&src.stages[1], &src.stages[2]);
        if m1.in_ch != 3 || m1.out_ch != 3 || m2.in_ch != 3 || m2.out_ch != 3 {
            return false;
        }
        let (StageData::Matrix { m: d1, off: o1 }, StageData::Matrix { m: d2, off: o2 }) =
            (&m1.data, &m2.data)
        else {
            return false;
        };
        if o1.is_some() {
            return false;
        }
        let res = mat3_per(&mat3_from_flat(d2), &mat3_from_flat(d1));
        let identity = mat3_is_identity(&res) && o2.is_none();
        (res, o2.clone(), identity, 0, 3)
    } else if src.check_stages(&[V, M, V]) {
        let m1 = &src.stages[1];
        if m1.in_ch != 3 || m1.out_ch != 3 {
            return false;
        }
        let StageData::Matrix { m: d, off: o } = &m1.data else {
            return false;
        };
        let res = mat3_from_flat(d);
        let identity = mat3_is_identity(&res) && o.is_none();
        (res, o.clone(), identity, 0, 2)
    } else {
        return false;
    };
    let Some(mut dest) = Pipeline::new(src.input_channels, src.output_channels) else {
        return false;
    };
    let curve1 = src.stages[c1].clone();
    let curve2 = src.stages[c2].clone();
    if !dest.insert_begin(Some(curve1.clone())) {
        return false;
    }
    if !identity && !dest.insert_end(Stage::matrix(3, 3, &mat3_flat(&res), offset.as_deref())) {
        return false;
    }
    if !dest.insert_end(Some(curve2.clone())) {
        return false;
    }
    if identity {
        optimize_by_joining_curves(&mut dest, input, output, flags);
    } else {
        *flags |= FLAGS_NOCACHE;
        let (Some(k1), Some(k2)) = (curve1.curves(), curve2.curves()) else {
            return false;
        };
        let is_8bits = formatter_is_8bit(*output);
        let mut mat = [[0i32; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                mat[i][j] = double_to_1fixed14(res[i][j]);
            }
        }
        let mut off = [0i32; 3];
        if let Some(o) = &offset {
            for i in 0..3 {
                off[i] = double_to_1fixed14(o[i]);
            }
        }
        if is_8bits {
            *output |= 1 << 21;
        }
        dest.opt = Some(Opt16::MatShaper(Box::new(MatShaper8 {
            shaper1: [
                fill_first_shaper(&k1[0]),
                fill_first_shaper(&k1[1]),
                fill_first_shaper(&k1[2]),
            ],
            mat,
            off,
            shaper2: [
                fill_second_shaper(&k2[0], is_8bits),
                fill_second_shaper(&k2[1], is_8bits),
                fill_second_shaper(&k2[2], is_8bits),
            ],
        })));
    }
    let _ = intent;
    *lut = dest;
    true
}

pub fn optimize_pipeline(
    lut: &mut Pipeline,
    intent: u32,
    input: &mut u32,
    output: &mut u32,
    flags: &mut u32,
) -> bool {
    if *flags & FLAGS_FORCE_CLUT != 0 {
        pre_optimize(lut);
        return optimize_by_resampling(lut, intent, input, output, flags);
    }
    if lut.stages.is_empty() {
        lut.opt = Some(Opt16::Identity);
        return true;
    }
    if lut.stages.iter().any(|s| s.typ == SIG_NAMED_COLOR_ELEM) {
        return false;
    }
    let any = pre_optimize(lut);
    if lut.stages.is_empty() {
        lut.opt = Some(Opt16::Identity);
        return true;
    }
    if *flags & FLAGS_NOOPTIMIZE != 0 {
        return any;
    }
    if optimize_by_joining_curves(lut, input, output, flags)
        || optimize_matrix_shaper(lut, intent, input, output, flags)
        || optimize_by_computing_linearization(lut, intent, input, output, flags)
        || optimize_by_resampling(lut, intent, input, output, flags)
    {
        return true;
    }
    any
}
