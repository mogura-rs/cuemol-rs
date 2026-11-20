//! ICC intents and profile linking (port of LittleCMS 2.19.1 cmscnvrt.c, MIT).

use super::base::*;
use super::io1::{
    read_chad, read_devicelink_lut, read_input_lut, read_media_white_point, read_output_lut,
};
use super::lut::{Pipeline, Stage};
use super::profile::Profile;
use super::samp::{detect_black_point, detect_destination_black_point};

fn compute_black_point_compensation(bp_in: &CieXyz, bp_out: &CieXyz) -> (Mat3, Vec3) {
    let tx = bp_in.x - D50.x;
    let ty = bp_in.y - D50.y;
    let tz = bp_in.z - D50.z;
    let ax = (bp_out.x - D50.x) / tx;
    let ay = (bp_out.y - D50.y) / ty;
    let az = (bp_out.z - D50.z) / tz;
    let bx = -D50.x * (bp_out.x - bp_in.x) / tx;
    let by = -D50.y * (bp_out.y - bp_in.y) / ty;
    let bz = -D50.z * (bp_out.z - bp_in.z) / tz;
    (
        [[ax, 0.0, 0.0], [0.0, ay, 0.0], [0.0, 0.0, az]],
        [bx, by, bz],
    )
}

/// Only full adaptation (state 1.0) is reachable: the adaptation state is never changed.
fn compute_absolute_intent(white_in: &CieXyz, white_out: &CieXyz) -> Mat3 {
    [
        [white_in.x / white_out.x, 0.0, 0.0],
        [0.0, white_in.y / white_out.y, 0.0],
        [0.0, 0.0, white_in.z / white_out.z],
    ]
}

fn is_empty_layer(m: &Mat3, off: &Vec3) -> bool {
    let id = mat3_identity();
    let mut diff = 0.0f64;
    for i in 0..3 {
        for j in 0..3 {
            diff += (m[i][j] - id[i][j]).abs();
        }
    }
    for v in off {
        diff += v.abs();
    }
    diff < 0.002
}

fn compute_conversion(
    i: usize,
    profiles: &[&Profile],
    intent: u32,
    bpc: bool,
) -> Option<(Mat3, Vec3)> {
    let mut m = mat3_identity();
    let mut off = [0.0f64; 3];
    if intent == INTENT_ABSOLUTE_COLORIMETRIC {
        let white_in = read_media_white_point(profiles[i - 1]);
        read_chad(profiles[i - 1])?;
        let white_out = read_media_white_point(profiles[i]);
        read_chad(profiles[i])?;
        m = compute_absolute_intent(&white_in, &white_out);
    } else if bpc {
        let bp_in = detect_black_point(profiles[i - 1], intent);
        let bp_out = detect_destination_black_point(profiles[i], intent);
        if bp_in != bp_out {
            (m, off) = compute_black_point_compensation(&bp_in, &bp_out);
        }
    }
    for v in &mut off {
        *v /= MAX_ENCODEABLE_XYZ;
    }
    Some((m, off))
}

fn add_conversion(result: &mut Pipeline, in_pcs: u32, out_pcs: u32, m: &Mat3, off: &Vec3) -> bool {
    let mflat = mat3_flat(m);
    let matrix = || Stage::matrix(3, 3, &mflat, Some(off));
    match in_pcs {
        SIG_XYZ_DATA => match out_pcs {
            SIG_XYZ_DATA => is_empty_layer(m, off) || result.insert_end(matrix()),
            SIG_LAB_DATA => {
                if !is_empty_layer(m, off) && !result.insert_end(matrix()) {
                    return false;
                }
                result.insert_end(Some(Stage::xyz2lab()))
            }
            _ => false,
        },
        SIG_LAB_DATA => match out_pcs {
            SIG_XYZ_DATA => {
                if !result.insert_end(Some(Stage::lab2xyz())) {
                    return false;
                }
                is_empty_layer(m, off) || result.insert_end(matrix())
            }
            SIG_LAB_DATA => {
                if !is_empty_layer(m, off)
                    && (!result.insert_end(Some(Stage::lab2xyz()))
                        || !result.insert_end(matrix())
                        || !result.insert_end(Some(Stage::xyz2lab())))
                {
                    return false;
                }
                true
            }
            _ => false,
        },
        _ => in_pcs == out_pcs,
    }
}

fn color_space_is_compatible(a: u32, b: u32) -> bool {
    a == b
        || (a == SIG_4COLOR_DATA && b == SIG_CMYK_DATA)
        || (a == SIG_CMYK_DATA && b == SIG_4COLOR_DATA)
        || (a == SIG_XYZ_DATA && b == SIG_LAB_DATA)
        || (a == SIG_LAB_DATA && b == SIG_XYZ_DATA)
}

fn default_icc_intents(profiles: &[&Profile], intents: &[u32], bpc: &[bool]) -> Option<Pipeline> {
    if profiles.is_empty() {
        return None;
    }
    let mut result = Pipeline::new(0, 0)?;
    let mut current = profiles[0].color_space;
    for (i, &p) in profiles.iter().enumerate() {
        let class = p.device_class;
        let is_device_link = class == SIG_LINK_CLASS || class == SIG_ABSTRACT_CLASS;
        let is_input = if i == 0 && !is_device_link {
            true
        } else {
            current != SIG_XYZ_DATA && current != SIG_LAB_DATA
        };
        let intent = intents[i];
        let (cs_in, cs_out) = if is_input || is_device_link {
            (p.color_space, p.pcs)
        } else {
            (p.pcs, p.color_space)
        };
        if !color_space_is_compatible(cs_in, current) {
            return None;
        }
        let lut;
        if is_device_link || (class == SIG_NAMED_COLOR_CLASS && profiles.len() == 1) {
            lut = read_devicelink_lut(p, intent)?;
            let (m, off) = if class == SIG_ABSTRACT_CLASS && i > 0 {
                compute_conversion(i, profiles, intent, bpc[i])?
            } else {
                (mat3_identity(), [0.0; 3])
            };
            if !add_conversion(&mut result, current, cs_in, &m, &off) {
                return None;
            }
        } else if is_input {
            lut = read_input_lut(p, intent)?;
        } else {
            lut = read_output_lut(p, intent)?;
            let (m, off) = compute_conversion(i, profiles, intent, bpc[i])?;
            if !add_conversion(&mut result, current, cs_in, &m, &off) {
                return None;
            }
        }
        if !result.cat(&lut) {
            return None;
        }
        current = cs_out;
    }
    Some(result)
}

/// _cmsLinkProfiles. `bpc` is updated in place, as in LittleCMS.
pub fn link_profiles(profiles: &[&Profile], intents: &[u32], bpc: &mut [bool]) -> Option<Pipeline> {
    if profiles.is_empty() || profiles.len() > 255 {
        return None;
    }
    for i in 0..profiles.len() {
        if intents[i] == INTENT_ABSOLUTE_COLORIMETRIC {
            bpc[i] = false;
        }
        if (intents[i] == INTENT_PERCEPTUAL || intents[i] == INTENT_SATURATION)
            && profiles[i].version >= 0x0400_0000
        {
            bpc[i] = true;
        }
    }
    if intents[0] > INTENT_ABSOLUTE_COLORIMETRIC {
        return None;
    }
    default_icc_intents(profiles, intents, bpc)
}
