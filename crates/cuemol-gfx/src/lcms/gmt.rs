//! Gamut check pipeline and RGB gamma detection (port of LittleCMS 2.19.1 cmsgmt.c, MIT).

use super::base::*;
use super::gamma::ToneCurve;
use super::io1::is_matrix_shaper;
use super::lut::{cube_size, Pipeline, Stage};
use super::profile::Profile;
use super::virt::{create_lab4, create_xyz};
use super::xform::{create_extended, create_transform, Transform};

const ERR_THRESHOLD: f64 = 5.0;

/// Upper bound on gamut grid nodes; LittleCMS would spend minutes on 9+ channel spaces.
const MAX_GAMUT_NODES: u32 = 7 * 7 * 7 * 7 * 7 * 7 * 7 * 7;

struct GamutChain {
    input: Transform,
    forward: Transform,
    reverse: Transform,
    threshold: f64,
}

fn gamut_sampler(t: &GamutChain, input: &[u16], out: &mut [u16]) -> bool {
    let lab_in1 = t.input.words_to_lab(input);
    let proof = t.forward.lab_to_words(&lab_in1);
    let lab_out1 = t.reverse.words_to_lab(&proof);
    let lab_in2 = lab_out1;
    let proof2 = t.forward.lab_to_words(&lab_out1);
    let lab_out2 = t.reverse.words_to_lab(&proof2);
    let de1 = delta_e(&lab_in1, &lab_out1);
    let de2 = delta_e(&lab_in2, &lab_out2);
    let thr = t.threshold;
    out[0] = if de1 < thr && de2 < thr {
        0
    } else if de1 < thr && de2 > thr {
        0
    } else if de1 > thr && de2 < thr {
        quick_floor((de1 - thr) + 0.5) as u16
    } else {
        let ratio = if de2 == 0.0 { de1 } else { de1 / de2 };
        if ratio > thr {
            quick_floor((ratio - thr) + 0.5) as u16
        } else {
            0
        }
    };
    true
}

pub fn create_gamut_check_pipeline(
    profiles: &[&Profile],
    bpc: &[bool],
    intents: &[u32],
    pcs_position: usize,
    gamut: &Profile,
) -> Option<Pipeline> {
    if pcs_position == 0 || pcs_position > 255 {
        return None;
    }
    let lab = create_lab4()?;
    let threshold = if is_matrix_shaper(gamut) {
        1.0
    } else {
        ERR_THRESHOLD
    };
    let mut list: Vec<&Profile> = profiles[..pcs_position].to_vec();
    list.push(&lab);
    let mut bpc_list: Vec<bool> = bpc[..pcs_position].to_vec();
    bpc_list.push(false);
    let mut intent_list: Vec<u32> = intents[..pcs_position].to_vec();
    intent_list.push(INTENT_RELATIVE_COLORIMETRIC);

    let color_space = gamut.color_space;
    let n_channels = channels_of_color_space(color_space);
    let n_grid = reasonable_gridpoints(color_space, FLAGS_HIGHRESPRECALC);
    let n_input = channels_of_color_space(list[0].color_space);

    let input_format = channels_sh(n_input as u32) | bytes_sh(2);
    let input = create_extended(
        &list,
        &mut bpc_list,
        &intent_list,
        None,
        0,
        input_format,
        TYPE_LAB_DBL,
        FLAGS_NOCACHE,
    );
    let device_format = channels_sh(n_channels as u32) | bytes_sh(2);
    let forward = create_transform(
        &lab,
        TYPE_LAB_DBL,
        gamut,
        device_format,
        INTENT_RELATIVE_COLORIMETRIC,
        FLAGS_NOCACHE,
    );
    let reverse = create_transform(
        gamut,
        device_format,
        &lab,
        TYPE_LAB_DBL,
        INTENT_RELATIVE_COLORIMETRIC,
        FLAGS_NOCACHE,
    );
    let chain = GamutChain {
        input: input?,
        forward: forward?,
        reverse: reverse?,
        threshold,
    };
    let mut pipeline = Pipeline::new(3, 1)?;
    let channels = usize::try_from(n_channels).ok()?;
    if channels <= MAX_INPUT_DIMENSIONS
        && cube_size(&[n_grid; MAX_INPUT_DIMENSIONS], channels) > MAX_GAMUT_NODES
    {
        return None;
    }
    if !pipeline.insert_begin(Stage::clut16(n_grid, channels, 1, None)) {
        return None;
    }
    pipeline.stages[0].sample_clut16(|i, o| gamut_sampler(&chain, i, o), false);
    Some(pipeline)
}

pub fn detect_rgb_profile_gamma(p: &Profile, threshold: f64) -> f64 {
    if p.color_space != SIG_RGB_DATA {
        return -1.0;
    }
    let cl = p.device_class;
    if cl != SIG_INPUT_CLASS
        && cl != SIG_DISPLAY_CLASS
        && cl != SIG_OUTPUT_CLASS
        && cl != SIG_COLORSPACE_CLASS
    {
        return -1.0;
    }
    let Some(xyz) = create_xyz() else {
        return -1.0;
    };
    let Some(xform) = create_transform(
        p,
        TYPE_RGB_16,
        &xyz,
        TYPE_XYZ_DBL,
        INTENT_RELATIVE_COLORIMETRIC,
        FLAGS_NOOPTIMIZE,
    ) else {
        return -1.0;
    };
    let y: Vec<f32> = (0..256)
        .map(|i| {
            let w = from_8_to_16(i as u8);
            xform.words_to_xyz(&[w, w, w]).y as f32
        })
        .collect();
    let Some(curve) = ToneCurve::tabulated_float(&y) else {
        return -1.0;
    };
    curve.estimate_gamma(threshold)
}
