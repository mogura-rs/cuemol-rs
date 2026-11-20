//! Transform creation and the pixel formats in use (port of LittleCMS 2.19.1
//! cmsxform.c and cmspack.c, MIT).

use super::base::*;
use super::cnvrt::link_profiles;
use super::gmt::{create_gamut_check_pipeline, detect_rgb_profile_gamma};
use super::lut::Pipeline;
use super::opt::optimize_pipeline;
use super::profile::Profile;

pub struct Transform {
    pub lut: Pipeline,
    pub input_format: u32,
    pub output_format: u32,
    pub gamut_check: Option<Pipeline>,
}

fn xform_color_spaces(profiles: &[&Profile]) -> Option<(u32, u32)> {
    let first = profiles.first()?;
    let mut post = first.color_space;
    let mut input = post;
    for (i, p) in profiles.iter().enumerate() {
        let is_input = post != SIG_XYZ_DATA && post != SIG_LAB_DATA;
        let class = p.device_class;
        let (cs_in, cs_out) = if class == SIG_NAMED_COLOR_CLASS {
            (
                sig(b"1CLR"),
                if profiles.len() > 1 {
                    p.pcs
                } else {
                    p.color_space
                },
            )
        } else if is_input || class == SIG_LINK_CLASS {
            (p.color_space, p.pcs)
        } else {
            (p.pcs, p.color_space)
        };
        if i == 0 {
            input = cs_in;
        }
        post = cs_out;
    }
    Some((input, post))
}

fn is_proper_color_space(check: u32, format: u32) -> bool {
    let space1 = t_colorspace(format);
    let space2 = lcms_color_space(check);
    if format == 0 {
        return true;
    }
    if space1 == 0 {
        return t_channels(format) == channels_of(check);
    }
    space1 == space2
        || (space1 == PT_LABV2 && space2 == PT_LAB)
        || (space1 == PT_LAB && space2 == PT_LABV2)
}

fn plain_words(format: u32) -> bool {
    t_float(format) == 0 && t_bytes(format) == 2 && format & 0x7F80 == 0
}

/// Formats with a formatter in this port.
fn formats_supported(input: u32, output: u32) -> bool {
    if formatter_is_float(input) || formatter_is_float(output) {
        (input == TYPE_LAB_DBL || plain_words(input))
            && (output == TYPE_LAB_DBL || output == TYPE_XYZ_DBL || plain_words(output))
    } else {
        input == TYPE_RGB_8 && output & !(1 << 21) == TYPE_RGB_8
    }
}

#[allow(clippy::too_many_arguments)]
pub fn create_extended(
    profiles: &[&Profile],
    bpc: &mut [bool],
    intents: &[u32],
    gamut_profile: Option<&Profile>,
    gamut_pcs_position: usize,
    input_format: u32,
    output_format: u32,
    mut flags: u32,
) -> Option<Transform> {
    let n = profiles.len();
    if n == 0 || n > 255 {
        return None;
    }
    if flags & FLAGS_GAMUTCHECK != 0 && gamut_profile.is_none() {
        flags &= !FLAGS_GAMUTCHECK;
    }
    if flags & FLAGS_GAMUTCHECK != 0 && (gamut_pcs_position == 0 || gamut_pcs_position >= n - 1) {
        return None;
    }
    if formatter_is_float(input_format) || formatter_is_float(output_format) {
        flags |= FLAGS_NOCACHE;
    }
    let (entry, exit) = xform_color_spaces(profiles)?;
    if !is_proper_color_space(entry, input_format) || !is_proper_color_space(exit, output_format) {
        return None;
    }
    if entry == SIG_RGB_DATA && t_bytes(input_format) == 2 && flags & FLAGS_NOOPTIMIZE == 0 {
        let gamma = detect_rgb_profile_gamma(profiles[0], 0.1);
        if gamma > 0.0 && gamma < 1.6 {
            flags |= FLAGS_NOOPTIMIZE;
        }
    }
    let mut lut = link_profiles(profiles, intents, bpc)?;
    if channels_of_color_space(entry) != lut.input_channels as i32
        || channels_of_color_space(exit) != lut.output_channels as i32
    {
        return None;
    }
    let mut input_format = input_format;
    let mut output_format = output_format;
    optimize_pipeline(
        &mut lut,
        intents[n - 1],
        &mut input_format,
        &mut output_format,
        &mut flags,
    );
    if !formats_supported(input_format, output_format) {
        return None;
    }
    let mut xform = Transform {
        lut,
        input_format,
        output_format,
        gamut_check: None,
    };
    if let Some(gamut) = gamut_profile {
        if flags & FLAGS_GAMUTCHECK != 0 {
            xform.gamut_check = Some(create_gamut_check_pipeline(
                profiles,
                bpc,
                intents,
                gamut_pcs_position,
                gamut,
            )?);
        }
    }
    Some(xform)
}

pub fn create_transform(
    input: &Profile,
    input_format: u32,
    output: &Profile,
    output_format: u32,
    intent: u32,
    flags: u32,
) -> Option<Transform> {
    let mut bpc = [false; 2];
    create_extended(
        &[input, output],
        &mut bpc,
        &[intent, intent],
        None,
        0,
        input_format,
        output_format,
        flags,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn create_proofing_transform(
    input: &Profile,
    input_format: u32,
    output: &Profile,
    output_format: u32,
    proofing: &Profile,
    intent: u32,
    proofing_intent: u32,
    flags: u32,
) -> Option<Transform> {
    let profiles = [input, proofing, proofing, output];
    let intents = [
        intent,
        intent,
        INTENT_RELATIVE_COLORIMETRIC,
        proofing_intent,
    ];
    let mut bpc = [false; 4];
    create_extended(
        &profiles,
        &mut bpc,
        &intents,
        Some(proofing),
        1,
        input_format,
        output_format,
        flags,
    )
}

impl Transform {
    fn eval_float_pixel(&self, input: &[f32; MAX_CHANNELS]) -> [f32; MAX_CHANNELS] {
        let mut out = [0f32; MAX_CHANNELS];
        self.lut.eval_float(input, &mut out);
        out
    }

    /// Unroll16ToFloat followed by PackLabDoubleFromFloat.
    pub fn words_to_lab(&self, words: &[u16]) -> CieLab {
        let mut fin = [0f32; MAX_CHANNELS];
        for i in 0..t_channels(self.input_format) as usize {
            fin[i] = words.get(i).copied().unwrap_or(0) as f32 / 65535.0f32;
        }
        let out = self.eval_float_pixel(&fin);
        CieLab {
            l: out[0] as f64 * 100.0,
            a: out[1] as f64 * 255.0 - 128.0,
            b: out[2] as f64 * 255.0 - 128.0,
        }
    }

    /// Unroll16ToFloat followed by PackXYZDoubleFromFloat.
    pub fn words_to_xyz(&self, words: &[u16]) -> CieXyz {
        let mut fin = [0f32; MAX_CHANNELS];
        for i in 0..t_channels(self.input_format) as usize {
            fin[i] = words.get(i).copied().unwrap_or(0) as f32 / 65535.0f32;
        }
        let out = self.eval_float_pixel(&fin);
        CieXyz {
            x: out[0] as f64 * MAX_ENCODEABLE_XYZ,
            y: out[1] as f64 * MAX_ENCODEABLE_XYZ,
            z: out[2] as f64 * MAX_ENCODEABLE_XYZ,
        }
    }

    /// UnrollLabDoubleToFloat followed by PackWordsFromFloat.
    pub fn lab_to_words(&self, lab: &CieLab) -> [u16; MAX_CHANNELS] {
        let mut fin = [0f32; MAX_CHANNELS];
        fin[0] = (lab.l / 100.0) as f32;
        fin[1] = ((lab.a + 128.0) / 255.0) as f32;
        fin[2] = ((lab.b + 128.0) / 255.0) as f32;
        let out = self.eval_float_pixel(&fin);
        let mut words = [0u16; MAX_CHANNELS];
        for i in 0..t_channels(self.output_format) as usize {
            words[i] = quick_saturate_word(out[i] as f64 * 65535.0);
        }
        words
    }
}
