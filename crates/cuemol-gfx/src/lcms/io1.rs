//! Profile LUT construction and info (port of LittleCMS 2.19.1 cmsio1.c, MIT).

use super::base::*;
use super::lut::{Pipeline, Stage, StageData, SIG_CLUT_ELEM};
use super::profile::*;

const DEVICE2PCS16: [u32; 4] = [SIG_A2B0, SIG_A2B1, SIG_A2B2, SIG_A2B1];
const DEVICE2PCS_FLOAT: [u32; 4] = [SIG_D2B0, SIG_D2B1, SIG_D2B2, SIG_D2B3];
const PCS2DEVICE16: [u32; 4] = [SIG_B2A0, SIG_B2A1, SIG_B2A2, SIG_B2A1];
const PCS2DEVICE_FLOAT: [u32; 4] = [SIG_B2D0, SIG_B2D1, SIG_B2D2, SIG_B2D3];

const INP_ADJ: f64 = 1.0 / MAX_ENCODEABLE_XYZ;
const OUTP_ADJ: f64 = MAX_ENCODEABLE_XYZ;

pub const USED_AS_INPUT: u32 = 0;
pub const USED_AS_OUTPUT: u32 = 1;

pub fn read_media_white_point(p: &Profile) -> CieXyz {
    let Some(tag) = p.read_xyz(SIG_MEDIA_WHITE_POINT) else {
        return D50;
    };
    if p.version < 0x0400_0000 && p.device_class == SIG_DISPLAY_CLASS {
        return D50;
    }
    tag
}

pub fn read_chad(p: &Profile) -> Option<Mat3> {
    if let Some(v) = p.read_tag(SIG_CHROMATIC_ADAPTATION) {
        if let TagValue::Doubles(d) = v.as_ref() {
            return Some(mat3_from_flat(d));
        }
    }
    if p.version < 0x0400_0000 && p.device_class == SIG_DISPLAY_CLASS {
        let Some(white) = p.read_xyz(SIG_MEDIA_WHITE_POINT) else {
            return Some(mat3_identity());
        };
        return adaptation_matrix(&white, &D50);
    }
    Some(mat3_identity())
}

fn read_icc_matrix_rgb2xyz(p: &Profile) -> Option<Mat3> {
    let r = p.read_xyz(SIG_RED_COLORANT)?;
    let g = p.read_xyz(SIG_GREEN_COLORANT)?;
    let b = p.read_xyz(SIG_BLUE_COLORANT)?;
    Some([[r.x, g.x, b.x], [r.y, g.y, b.y], [r.z, g.z, b.z]])
}

fn build_gray_input_matrix_pipeline(p: &Profile) -> Option<Pipeline> {
    let trc = p.read_curve(SIG_GRAY_TRC)?;
    let gray = as_curve(&trc);
    let mut lut = Pipeline::new(1, 3)?;
    if p.pcs == SIG_LAB_DATA {
        let empty = super::gamma::ToneCurve::tabulated16(2, Some(&[0x8080, 0x8080]))?;
        let curves = [gray.clone(), empty.clone(), empty];
        if !lut.insert_end(Stage::matrix(3, 1, &[1.0, 1.0, 1.0], None))
            || !lut.insert_end(Stage::tone_curves(3, Some(&curves)))
        {
            return None;
        }
    } else {
        let m = [INP_ADJ * D50_X, INP_ADJ * D50_Y, INP_ADJ * D50_Z];
        if !lut.insert_end(Stage::tone_curves(1, Some(std::slice::from_ref(gray))))
            || !lut.insert_end(Stage::matrix(3, 1, &m, None))
        {
            return None;
        }
    }
    Some(lut)
}

fn build_rgb_input_matrix_shaper(p: &Profile) -> Option<Pipeline> {
    let mut mat = read_icc_matrix_rgb2xyz(p)?;
    for row in &mut mat {
        for v in row.iter_mut() {
            *v *= INP_ADJ;
        }
    }
    let r = p.read_curve(SIG_RED_TRC)?;
    let g = p.read_curve(SIG_GREEN_TRC)?;
    let b = p.read_curve(SIG_BLUE_TRC)?;
    let shapes = [
        as_curve(&r).clone(),
        as_curve(&g).clone(),
        as_curve(&b).clone(),
    ];
    let mut lut = Pipeline::new(3, 3)?;
    if !lut.insert_end(Stage::tone_curves(3, Some(&shapes)))
        || !lut.insert_end(Stage::matrix(3, 3, &mat3_flat(&mat), None))
    {
        return None;
    }
    if p.pcs == SIG_LAB_DATA && !lut.insert_end(Some(Stage::xyz2lab())) {
        return None;
    }
    Some(lut)
}

fn read_pipeline(p: &Profile, s: u32) -> Option<(u32, Pipeline)> {
    let (typ, v) = p.read_tag_typed(s)?;
    match v.as_ref() {
        TagValue::Pipeline(lut) => Some((typ, lut.clone())),
        _ => None,
    }
}

fn float_lut(p: &Profile, tag: u32, input: u32, output: u32) -> Option<Pipeline> {
    let (_, mut lut) = read_pipeline(p, tag)?;
    if (input == SIG_LAB_DATA && !lut.insert_begin(Stage::normalize_to_lab_float()))
        || (input == SIG_XYZ_DATA && !lut.insert_begin(Stage::normalize_to_xyz_float()))
        || (output == SIG_LAB_DATA && !lut.insert_end(Stage::normalize_from_lab_float()))
        || (output == SIG_XYZ_DATA && !lut.insert_end(Stage::normalize_from_xyz_float()))
    {
        return None;
    }
    Some(lut)
}

pub fn read_input_lut(p: &Profile, intent: u32) -> Option<Pipeline> {
    if p.device_class == SIG_NAMED_COLOR_CLASS {
        return None;
    }
    if intent <= INTENT_ABSOLUTE_COLORIMETRIC {
        let mut tag16 = DEVICE2PCS16[intent as usize];
        let tag_float = DEVICE2PCS_FLOAT[intent as usize];
        if p.is_tag(tag_float) {
            return float_lut(p, tag_float, p.color_space, p.pcs);
        }
        if !p.is_tag(tag16) {
            tag16 = DEVICE2PCS16[0];
        }
        if p.is_tag(tag16) {
            let (original, mut lut) = read_pipeline(p, tag16)?;
            if original != TYPE_LUT16 || p.pcs != SIG_LAB_DATA {
                return Some(lut);
            }
            if p.color_space == SIG_LAB_DATA && !lut.insert_begin(Stage::lab_v4_to_v2()) {
                return None;
            }
            if !lut.insert_end(Stage::lab_v2_to_v4()) {
                return None;
            }
            return Some(lut);
        }
    }
    if p.color_space == SIG_GRAY_DATA {
        return build_gray_input_matrix_pipeline(p);
    }
    build_rgb_input_matrix_shaper(p)
}

fn build_gray_output_pipeline(p: &Profile) -> Option<Pipeline> {
    let trc = p.read_curve(SIG_GRAY_TRC)?;
    let rev = as_curve(&trc).reverse()?;
    let mut lut = Pipeline::new(3, 1)?;
    let m = if p.pcs == SIG_LAB_DATA {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, OUTP_ADJ * D50_Y, 0.0]
    };
    if !lut.insert_end(Stage::matrix(1, 3, &m, None)) {
        return None;
    }
    if !lut.insert_end(Stage::tone_curves(1, Some(&[rev]))) {
        return None;
    }
    Some(lut)
}

fn build_rgb_output_matrix_shaper(p: &Profile) -> Option<Pipeline> {
    let mat = read_icc_matrix_rgb2xyz(p)?;
    let mut inv = mat3_inverse(&mat)?;
    for row in &mut inv {
        for v in row.iter_mut() {
            *v *= OUTP_ADJ;
        }
    }
    let r = p.read_curve(SIG_RED_TRC)?;
    let g = p.read_curve(SIG_GREEN_TRC)?;
    let b = p.read_curve(SIG_BLUE_TRC)?;
    let inv_shapes = [
        as_curve(&r).reverse()?,
        as_curve(&g).reverse()?,
        as_curve(&b).reverse()?,
    ];
    let mut lut = Pipeline::new(3, 3)?;
    if p.pcs == SIG_LAB_DATA && !lut.insert_end(Some(Stage::lab2xyz())) {
        return None;
    }
    if !lut.insert_end(Stage::matrix(3, 3, &mat3_flat(&inv), None))
        || !lut.insert_end(Stage::tone_curves(3, Some(&inv_shapes)))
    {
        return None;
    }
    Some(lut)
}

fn change_interpolation_to_trilinear(lut: &mut Pipeline) {
    for s in &mut lut.stages {
        if s.typ == SIG_CLUT_ELEM {
            if let StageData::Clut16 { params, .. } = &mut s.data {
                params.trilinear = true;
            }
        }
    }
}

pub fn read_output_lut(p: &Profile, intent: u32) -> Option<Pipeline> {
    if intent <= INTENT_ABSOLUTE_COLORIMETRIC {
        let mut tag16 = PCS2DEVICE16[intent as usize];
        let tag_float = PCS2DEVICE_FLOAT[intent as usize];
        if p.is_tag(tag_float) {
            return float_lut(p, tag_float, p.pcs, p.color_space);
        }
        if !p.is_tag(tag16) {
            tag16 = PCS2DEVICE16[0];
        }
        if p.is_tag(tag16) {
            let (original, mut lut) = read_pipeline(p, tag16)?;
            if p.pcs == SIG_LAB_DATA {
                change_interpolation_to_trilinear(&mut lut);
            }
            if original != TYPE_LUT16 || p.pcs != SIG_LAB_DATA {
                return Some(lut);
            }
            if !lut.insert_begin(Stage::lab_v4_to_v2()) {
                return None;
            }
            if p.color_space == SIG_LAB_DATA && !lut.insert_end(Stage::lab_v2_to_v4()) {
                return None;
            }
            return Some(lut);
        }
    }
    if p.color_space == SIG_GRAY_DATA {
        return build_gray_output_pipeline(p);
    }
    build_rgb_output_matrix_shaper(p)
}

pub fn read_devicelink_lut(p: &Profile, intent: u32) -> Option<Pipeline> {
    if intent > INTENT_ABSOLUTE_COLORIMETRIC {
        return None;
    }
    let mut tag16 = DEVICE2PCS16[intent as usize];
    if p.device_class == SIG_NAMED_COLOR_CLASS {
        return None;
    }
    let tag_float = DEVICE2PCS_FLOAT[intent as usize];
    if p.is_tag(tag_float) {
        return float_lut(p, tag_float, p.color_space, p.pcs);
    }
    if p.is_tag(DEVICE2PCS_FLOAT[0]) {
        return read_pipeline(p, DEVICE2PCS_FLOAT[0]).map(|(_, lut)| lut);
    }
    if !p.is_tag(tag16) {
        tag16 = DEVICE2PCS16[0];
        if !p.is_tag(tag16) {
            return None;
        }
    }
    let (original, mut lut) = read_pipeline(p, tag16)?;
    if p.pcs == SIG_LAB_DATA {
        change_interpolation_to_trilinear(&mut lut);
    }
    if original != TYPE_LUT16 {
        return Some(lut);
    }
    if p.color_space == SIG_LAB_DATA && !lut.insert_begin(Stage::lab_v4_to_v2()) {
        return None;
    }
    if p.pcs == SIG_LAB_DATA && !lut.insert_end(Stage::lab_v2_to_v4()) {
        return None;
    }
    Some(lut)
}

pub fn is_matrix_shaper(p: &Profile) -> bool {
    match p.color_space {
        SIG_GRAY_DATA => p.is_tag(SIG_GRAY_TRC),
        SIG_RGB_DATA => [
            SIG_RED_COLORANT,
            SIG_GREEN_COLORANT,
            SIG_BLUE_COLORANT,
            SIG_RED_TRC,
            SIG_GREEN_TRC,
            SIG_BLUE_TRC,
        ]
        .iter()
        .all(|&t| p.is_tag(t)),
        _ => false,
    }
}

pub fn is_clut(p: &Profile, intent: u32, direction: u32) -> bool {
    if p.device_class == SIG_LINK_CLASS {
        return p.rendering_intent == intent;
    }
    let table = match direction {
        USED_AS_INPUT => &DEVICE2PCS16,
        USED_AS_OUTPUT => &PCS2DEVICE16,
        _ => return false,
    };
    if intent > INTENT_ABSOLUTE_COLORIMETRIC {
        return false;
    }
    p.is_tag(table[intent as usize])
}

pub fn is_intent_supported(p: &Profile, intent: u32, direction: u32) -> bool {
    is_clut(p, intent, direction) || is_matrix_shaper(p)
}

/// ASCII bytes of the description (0), manufacturer (1), model (2) or copyright (3).
pub fn profile_info_ascii(p: &Profile, info: u32) -> Option<Vec<u8>> {
    let s = match info {
        0 => {
            if p.is_tag(SIG_PROFILE_DESCRIPTION_ML) {
                SIG_PROFILE_DESCRIPTION_ML
            } else {
                SIG_PROFILE_DESCRIPTION
            }
        }
        1 => SIG_DEVICE_MFG_DESC,
        2 => SIG_DEVICE_MODEL_DESC,
        3 => SIG_COPYRIGHT,
        _ => return None,
    };
    match p.read_tag(s)?.as_ref() {
        TagValue::Mlu(mlu) => mlu.get_ascii(b"en", b"US"),
        _ => None,
    }
}
