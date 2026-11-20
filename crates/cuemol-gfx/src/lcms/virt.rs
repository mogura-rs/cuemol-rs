//! Built-in virtual profiles (port of LittleCMS 2.19.1 cmsvirt.c, MIT).

use super::base::*;
use super::gamma::ToneCurve;
use super::lut::{Pipeline, Stage};
use super::profile::*;

fn curve_type(version: f64, curve: &ToneCurve) -> u32 {
    if version < 4.0
        || curve.segments.len() != 1
        || curve.segments[0].typ < 0
        || curve.segments[0].typ > 5
    {
        TYPE_CURVE
    } else {
        TYPE_PARAMETRIC_CURVE
    }
}

fn lut_a2b_type(version: f64) -> u32 {
    if version < 4.0 {
        TYPE_LUT16
    } else {
        TYPE_LUT_A2B
    }
}

fn create_rgb_profile(
    white: Option<&CieXyY>,
    primaries: Option<&[CieXyY; 3]>,
    transfer: Option<&ToneCurve>,
) -> Option<Profile> {
    let mut p = Profile::placeholder();
    p.set_version(4.4);
    p.device_class = SIG_DISPLAY_CLASS;
    p.color_space = SIG_RGB_DATA;
    p.pcs = SIG_XYZ_DATA;
    p.rendering_intent = INTENT_PERCEPTUAL;
    if let Some(white) = white {
        p.write_tag(SIG_MEDIA_WHITE_POINT, TYPE_XYZ, TagValue::Xyz(D50));
        let white_xyz = xyy2xyz(white);
        let chad = adaptation_matrix(&white_xyz, &D50).unwrap_or_else(mat3_identity);
        p.write_tag(
            SIG_CHROMATIC_ADAPTATION,
            TYPE_S15FIXED16_ARRAY,
            TagValue::Doubles(mat3_flat(&chad).to_vec()),
        );
        if let Some(primaries) = primaries {
            let max_white = CieXyY {
                x: white.x,
                y: white.y,
                big_y: 1.0,
            };
            let m = build_rgb2xyz_transfer_matrix(&max_white, primaries)?;
            let col = |j: usize| CieXyz {
                x: m[0][j],
                y: m[1][j],
                z: m[2][j],
            };
            p.write_tag(SIG_RED_COLORANT, TYPE_XYZ, TagValue::Xyz(col(0)));
            p.write_tag(SIG_BLUE_COLORANT, TYPE_XYZ, TagValue::Xyz(col(2)));
            p.write_tag(SIG_GREEN_COLORANT, TYPE_XYZ, TagValue::Xyz(col(1)));
        }
    }
    if let Some(trc) = transfer {
        let typ = curve_type(p.profile_version(), trc);
        for s in [SIG_RED_TRC, SIG_GREEN_TRC, SIG_BLUE_TRC] {
            p.write_tag(s, typ, TagValue::Curve(trc.clone()));
        }
    }
    Some(p)
}

pub fn create_srgb() -> Option<Profile> {
    let d65 = CieXyY {
        x: 0.3127,
        y: 0.3290,
        big_y: 1.0,
    };
    let primaries = [
        CieXyY {
            x: 0.6400,
            y: 0.3300,
            big_y: 1.0,
        },
        CieXyY {
            x: 0.3000,
            y: 0.6000,
            big_y: 1.0,
        },
        CieXyY {
            x: 0.1500,
            y: 0.0600,
            big_y: 1.0,
        },
    ];
    let params = [2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045];
    let gamma = ToneCurve::parametric(4, &params)?;
    create_rgb_profile(Some(&d65), Some(&primaries), Some(&gamma))
}

fn identity_abstract(
    white: Option<&CieXyY>,
    version: f64,
    space: u32,
    stage: Stage,
) -> Option<Profile> {
    let mut p = create_rgb_profile(white, None, None)?;
    p.set_version(version);
    p.device_class = SIG_ABSTRACT_CLASS;
    p.color_space = space;
    p.pcs = space;
    let mut lut = Pipeline::new(3, 3)?;
    if !lut.insert_begin(Some(stage)) {
        return None;
    }
    p.write_tag(
        SIG_A2B0,
        lut_a2b_type(p.profile_version()),
        TagValue::Pipeline(lut),
    );
    Some(p)
}

/// cmsCreateLab2Profile(NULL)
pub fn create_lab2() -> Option<Profile> {
    identity_abstract(
        Some(&d50_xyy()),
        2.1,
        SIG_LAB_DATA,
        Stage::identity_clut(3)?,
    )
}

/// cmsCreateLab4Profile(NULL)
pub fn create_lab4() -> Option<Profile> {
    let mut p = identity_abstract(None, 4.4, SIG_LAB_DATA, Stage::identity_curves(3)?)?;
    p.write_tag(SIG_MEDIA_WHITE_POINT, TYPE_XYZ, TagValue::Xyz(D50));
    Some(p)
}

/// cmsCreateXYZProfile()
pub fn create_xyz() -> Option<Profile> {
    identity_abstract(
        Some(&d50_xyy()),
        4.4,
        SIG_XYZ_DATA,
        Stage::identity_curves(3)?,
    )
}
