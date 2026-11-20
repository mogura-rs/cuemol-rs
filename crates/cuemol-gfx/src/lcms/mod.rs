//! Pure Rust port of the subset of LittleCMS 2.19.1 (MIT, Marti Maria Saguer) used for
//! soft proofing an sRGB display through a user ICC profile.

mod base;
mod cnvrt;
mod gamma;
mod gmt;
mod intrp;
mod io1;
mod lut;
mod mpe;
mod opt;
mod profile;
mod samp;
mod virt;
mod xform;

use base::{
    from_16_to_8, from_8_to_16, FLAGS_GAMUTCHECK, FLAGS_NOCACHE, FLAGS_SOFTPROOFING,
    INTENT_ABSOLUTE_COLORIMETRIC, MAX_CHANNELS, TYPE_RGB_8,
};
use lut::Pipeline;

/// A soft-proofing transform sRGB -> proof profile -> sRGB with a gamut alarm.
pub struct SoftProof {
    lut: Pipeline,
    gamut: Pipeline,
    info: String,
}

impl SoftProof {
    pub fn new(bytes: &[u8], intent: u32) -> Option<SoftProof> {
        let proof = profile::Profile::open_mem(bytes)?;
        let srgb = virt::create_srgb()?;
        // The plain proofing transform shares this pipeline: only the gamut flag differs.
        let xform = xform::create_proofing_transform(
            &srgb,
            TYPE_RGB_8,
            &srgb,
            TYPE_RGB_8,
            &proof,
            intent,
            INTENT_ABSOLUTE_COLORIMETRIC,
            FLAGS_SOFTPROOFING | FLAGS_GAMUTCHECK | FLAGS_NOCACHE,
        )?;
        let info = (0..4)
            .filter_map(|kind| {
                let mut bytes = io1::profile_info_ascii(&proof, kind)?;
                if let Some(end) = bytes.iter().position(|&b| b == 0) {
                    bytes.truncate(end);
                }
                let text = String::from_utf8_lossy(&bytes).trim().to_string();
                (!text.is_empty()).then_some(text)
            })
            .collect::<Vec<_>>()
            .join("\n");
        Some(SoftProof {
            lut: xform.lut,
            gamut: xform.gamut_check?,
            info,
        })
    }

    pub fn info(&self) -> &str {
        &self.info
    }

    fn input(rgb: u32) -> [u16; MAX_CHANNELS] {
        let mut w = [0u16; MAX_CHANNELS];
        w[0] = from_8_to_16((rgb >> 16) as u8);
        w[1] = from_8_to_16((rgb >> 8) as u8);
        w[2] = from_8_to_16(rgb as u8);
        w
    }

    fn pack(w: &[u16]) -> u32 {
        ((from_16_to_8(w[0]) as u32) << 16)
            | ((from_16_to_8(w[1]) as u32) << 8)
            | from_16_to_8(w[2]) as u32
    }

    pub fn color(&self, rgb: u32) -> u32 {
        let mut out = [0u16; MAX_CHANNELS];
        self.lut.eval16(&Self::input(rgb), &mut out);
        Self::pack(&out)
    }

    /// Mirrors the alarm-code comparison used with cmsFLAGS_GAMUTCHECK.
    pub fn in_gamut(&self, rgb: u32) -> bool {
        let input = Self::input(rgb);
        let magenta = rgb & 0x00ff_ffff == 0x00ff_00ff;
        let alarm: [u16; 3] = if magenta {
            [0, 65535, 65535]
        } else {
            [65535, 0, 65535]
        };
        let mut out_of_gamut = [0u16; MAX_CHANNELS];
        self.gamut.eval16(&input, &mut out_of_gamut);
        let mut out = [0u16; MAX_CHANNELS];
        if out_of_gamut[0] >= 1 {
            for (i, o) in out.iter_mut().enumerate().take(self.lut.output_channels) {
                *o = alarm.get(i).copied().unwrap_or(0);
            }
        } else {
            self.lut.eval16(&input, &mut out);
        }
        let packed = Self::pack(&out);
        packed != ((alarm[0] as u32 >> 8) << 16 | (alarm[1] as u32 >> 8) << 8 | 255)
    }
}
