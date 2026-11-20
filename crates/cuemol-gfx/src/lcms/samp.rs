//! Black point detection (port of LittleCMS 2.19.1 cmssamp.c, MIT).
//!
//! Black point compensation is only ever forced for V4 profiles under the perceptual
//! and saturation intents (the bridge never sets cmsFLAGS_BLACKPOINTCOMPENSATION),
//! so only the V4 branches of the detectors are reachable and ported.

use super::base::*;
use super::io1::{is_intent_supported, is_matrix_shaper, USED_AS_INPUT};
use super::profile::Profile;
use super::virt::create_lab2;
use super::xform::create_transform;

const ZERO: CieXyz = CieXyz {
    x: 0.0,
    y: 0.0,
    z: 0.0,
};

fn black_point_as_darker_colorant(input: &Profile, intent: u32) -> CieXyz {
    if !is_intent_supported(input, intent, USED_AS_INPUT) {
        return ZERO;
    }
    let format = formatter_for_color_space(input.color_space, 2, false);
    let Some((_, black)) = end_points_by_space(input.color_space) else {
        return ZERO;
    };
    if black.len() as u32 != t_channels(format) {
        return ZERO;
    }
    let Some(lab_profile) = create_lab2() else {
        return ZERO;
    };
    let Some(xform) = create_transform(
        input,
        format,
        &lab_profile,
        TYPE_LAB_DBL,
        intent,
        FLAGS_NOOPTIMIZE | FLAGS_NOCACHE,
    ) else {
        return ZERO;
    };
    let mut lab = xform.words_to_lab(black);
    if lab.l > 95.0 {
        lab.l = 0.0;
    } else if lab.l < 0.0 {
        lab.l = 0.0;
    } else if lab.l > 50.0 {
        lab.l = 50.0;
    }
    lab2xyz(None, &lab)
}

fn v4_black_point(p: &Profile, intent: u32) -> CieXyz {
    let class = p.device_class;
    if class == SIG_LINK_CLASS || class == SIG_ABSTRACT_CLASS || class == SIG_NAMED_COLOR_CLASS {
        return ZERO;
    }
    if intent != INTENT_PERCEPTUAL
        && intent != INTENT_RELATIVE_COLORIMETRIC
        && intent != INTENT_SATURATION
    {
        return ZERO;
    }
    if is_matrix_shaper(p) {
        return black_point_as_darker_colorant(p, INTENT_RELATIVE_COLORIMETRIC);
    }
    PERCEPTUAL_BLACK
}

/// cmsDetectBlackPoint for a V4 profile and the perceptual or saturation intent.
pub fn detect_black_point(p: &Profile, intent: u32) -> CieXyz {
    v4_black_point(p, intent)
}

/// cmsDetectDestinationBlackPoint for a V4 profile and the perceptual or saturation intent.
pub fn detect_destination_black_point(p: &Profile, intent: u32) -> CieXyz {
    v4_black_point(p, intent)
}
