//! Shared constants, fixed point helpers, matrices and PCS math (LittleCMS 2.19.1, MIT).

pub const fn sig(b: &[u8; 4]) -> u32 {
    ((b[0] as u32) << 24) | ((b[1] as u32) << 16) | ((b[2] as u32) << 8) | b[3] as u32
}

pub const MAX_CHANNELS: usize = 16;
pub const MAX_INPUT_DIMENSIONS: usize = 15;
pub const MAX_STAGE_CHANNELS: usize = 128;
pub const MAX_ENCODEABLE_XYZ: f64 = 1.0 + 32767.0 / 32768.0;
pub const MATRIX_DET_TOLERANCE: f64 = 0.0001;

pub const D50_X: f64 = 0.9642;
pub const D50_Y: f64 = 1.0;
pub const D50_Z: f64 = 0.8249;
pub const D50: CieXyz = CieXyz {
    x: D50_X,
    y: D50_Y,
    z: D50_Z,
};
pub const PERCEPTUAL_BLACK: CieXyz = CieXyz {
    x: 0.00336,
    y: 0.0034731,
    z: 0.00287,
};

pub const INTENT_PERCEPTUAL: u32 = 0;
pub const INTENT_RELATIVE_COLORIMETRIC: u32 = 1;
pub const INTENT_SATURATION: u32 = 2;
pub const INTENT_ABSOLUTE_COLORIMETRIC: u32 = 3;

pub const FLAGS_NOWHITEONWHITEFIXUP: u32 = 0x0004;
pub const FLAGS_NOCACHE: u32 = 0x0040;
pub const FLAGS_NOOPTIMIZE: u32 = 0x0100;
pub const FLAGS_HIGHRESPRECALC: u32 = 0x0400;
pub const FLAGS_LOWRESPRECALC: u32 = 0x0800;
pub const FLAGS_GAMUTCHECK: u32 = 0x1000;
pub const FLAGS_SOFTPROOFING: u32 = 0x4000;
pub const FLAGS_FORCE_CLUT: u32 = 0x0002;
pub const FLAGS_CLUT_POST_LINEARIZATION: u32 = 0x0001;
pub const FLAGS_CLUT_PRE_LINEARIZATION: u32 = 0x0010;

// Color spaces
pub const SIG_XYZ_DATA: u32 = sig(b"XYZ ");
pub const SIG_LAB_DATA: u32 = sig(b"Lab ");
pub const SIG_LUV_DATA: u32 = sig(b"Luv ");
pub const SIG_YCBCR_DATA: u32 = sig(b"YCbr");
pub const SIG_YXY_DATA: u32 = sig(b"Yxy ");
pub const SIG_RGB_DATA: u32 = sig(b"RGB ");
pub const SIG_GRAY_DATA: u32 = sig(b"GRAY");
pub const SIG_HSV_DATA: u32 = sig(b"HSV ");
pub const SIG_HLS_DATA: u32 = sig(b"HLS ");
pub const SIG_CMYK_DATA: u32 = sig(b"CMYK");
pub const SIG_CMY_DATA: u32 = sig(b"CMY ");
pub const SIG_LUVK_DATA: u32 = sig(b"LuvK");
pub const SIG_4COLOR_DATA: u32 = sig(b"4CLR");

// Device classes
pub const SIG_INPUT_CLASS: u32 = sig(b"scnr");
pub const SIG_DISPLAY_CLASS: u32 = sig(b"mntr");
pub const SIG_OUTPUT_CLASS: u32 = sig(b"prtr");
pub const SIG_LINK_CLASS: u32 = sig(b"link");
pub const SIG_ABSTRACT_CLASS: u32 = sig(b"abst");
pub const SIG_COLORSPACE_CLASS: u32 = sig(b"spac");
pub const SIG_NAMED_COLOR_CLASS: u32 = sig(b"nmcl");
pub const SIG_COLOR_ENCODING_CLASS: u32 = sig(b"cenc");
pub const SIG_MULTIPLEX_ID_CLASS: u32 = sig(b"mid ");
pub const SIG_MULTIPLEX_LINK_CLASS: u32 = sig(b"mlnk");
pub const SIG_MULTIPLEX_VIS_CLASS: u32 = sig(b"mvis");

fn multi_from_any(cs: u32) -> Option<u32> {
    let digit = |c: u32| match c {
        0x31..=0x39 => Some(c - 0x30),
        0x41..=0x46 => Some(c - 0x41 + 10),
        _ => None,
    };
    if cs & 0xFFFF_FF00 == sig(b"MCH\0") {
        return digit(cs & 0xFF);
    }
    if cs & 0x00FF_FFFF == sig(b"\0CLR") {
        return digit(cs >> 24);
    }
    None
}

pub fn channels_of_color_space(cs: u32) -> i32 {
    match cs {
        SIG_GRAY_DATA => 1,
        SIG_XYZ_DATA | SIG_LAB_DATA | SIG_LUV_DATA | SIG_YCBCR_DATA | SIG_YXY_DATA
        | SIG_RGB_DATA | SIG_HSV_DATA | SIG_HLS_DATA | SIG_CMY_DATA => 3,
        SIG_LUVK_DATA | SIG_CMYK_DATA => 4,
        _ => multi_from_any(cs).map_or(-1, |n| n as i32),
    }
}

pub fn channels_of(cs: u32) -> u32 {
    let n = channels_of_color_space(cs);
    if n < 0 {
        3
    } else {
        n as u32
    }
}

pub const PT_GRAY: u32 = 3;
pub const PT_RGB: u32 = 4;
pub const PT_CMY: u32 = 5;
pub const PT_CMYK: u32 = 6;
pub const PT_YCBCR: u32 = 7;
pub const PT_YUV: u32 = 8;
pub const PT_XYZ: u32 = 9;
pub const PT_LAB: u32 = 10;
pub const PT_YUVK: u32 = 11;
pub const PT_HSV: u32 = 12;
pub const PT_HLS: u32 = 13;
pub const PT_YXY: u32 = 14;
pub const PT_MCH1: u32 = 15;
pub const PT_LABV2: u32 = 30;

pub fn lcms_color_space(cs: u32) -> u32 {
    match cs {
        SIG_GRAY_DATA => PT_GRAY,
        SIG_RGB_DATA => PT_RGB,
        SIG_CMY_DATA => PT_CMY,
        SIG_CMYK_DATA => PT_CMYK,
        SIG_YCBCR_DATA => PT_YCBCR,
        SIG_LUV_DATA => PT_YUV,
        SIG_XYZ_DATA => PT_XYZ,
        SIG_LAB_DATA => PT_LAB,
        SIG_LUVK_DATA => PT_YUVK,
        SIG_HSV_DATA => PT_HSV,
        SIG_HLS_DATA => PT_HLS,
        SIG_YXY_DATA => PT_YXY,
        _ => multi_from_any(cs).map_or(0, |n| PT_MCH1 + n - 1),
    }
}

pub fn icc_color_space(notation: u32) -> u32 {
    match notation {
        1 | PT_GRAY => SIG_GRAY_DATA,
        2 | PT_RGB => SIG_RGB_DATA,
        PT_CMY => SIG_CMY_DATA,
        PT_CMYK => SIG_CMYK_DATA,
        PT_YCBCR => SIG_YCBCR_DATA,
        PT_YUV => SIG_LUV_DATA,
        PT_XYZ => SIG_XYZ_DATA,
        PT_LAB | PT_LABV2 => SIG_LAB_DATA,
        PT_YUVK => SIG_LUVK_DATA,
        PT_HSV => SIG_HSV_DATA,
        PT_HLS => SIG_HLS_DATA,
        PT_YXY => SIG_YXY_DATA,
        15..=29 => {
            let n = notation - PT_MCH1 + 1;
            let c = if n < 10 { 0x30 + n } else { 0x41 + n - 10 };
            sig(b"MCH\0") | c
        }
        _ => 0,
    }
}

pub fn reasonable_gridpoints(cs: u32, flags: u32) -> u32 {
    if flags & 0x00FF_0000 != 0 {
        return (flags >> 16) & 0xFF;
    }
    let n = channels_of(cs);
    if flags & FLAGS_HIGHRESPRECALC != 0 {
        if n > 4 {
            return 7;
        }
        if n == 4 {
            return 23;
        }
        return 49;
    }
    if flags & FLAGS_LOWRESPRECALC != 0 {
        if n > 4 {
            return 6;
        }
        if n == 1 {
            return 33;
        }
        return 17;
    }
    if n > 4 {
        return 7;
    }
    if n == 4 {
        return 17;
    }
    33
}

/// White and black endpoints for the most common spaces.
pub fn end_points_by_space(cs: u32) -> Option<(&'static [u16], &'static [u16])> {
    static RGB_BLACK: [u16; 3] = [0, 0, 0];
    static RGB_WHITE: [u16; 3] = [0xffff, 0xffff, 0xffff];
    static CMYK_BLACK: [u16; 4] = [0xffff, 0xffff, 0xffff, 0xffff];
    static CMYK_WHITE: [u16; 4] = [0, 0, 0, 0];
    static LAB_BLACK: [u16; 3] = [0, 0x8080, 0x8080];
    static LAB_WHITE: [u16; 3] = [0xFFFF, 0x8080, 0x8080];
    static CMY_BLACK: [u16; 3] = [0xffff, 0xffff, 0xffff];
    static CMY_WHITE: [u16; 3] = [0, 0, 0];
    static GRAY_BLACK: [u16; 1] = [0];
    static GRAY_WHITE: [u16; 1] = [0xffff];
    match cs {
        SIG_GRAY_DATA => Some((&GRAY_WHITE, &GRAY_BLACK)),
        SIG_RGB_DATA => Some((&RGB_WHITE, &RGB_BLACK)),
        SIG_LAB_DATA => Some((&LAB_WHITE, &LAB_BLACK)),
        SIG_CMYK_DATA => Some((&CMYK_WHITE, &CMYK_BLACK)),
        SIG_CMY_DATA => Some((&CMY_WHITE, &CMY_BLACK)),
        _ => None,
    }
}

// Pixel format descriptors
pub const fn float_sh(a: u32) -> u32 {
    a << 22
}
pub const fn colorspace_sh(s: u32) -> u32 {
    s << 16
}
pub const fn channels_sh(c: u32) -> u32 {
    c << 3
}
pub const fn bytes_sh(b: u32) -> u32 {
    b
}
pub const fn t_float(f: u32) -> u32 {
    (f >> 22) & 1
}
pub const fn t_colorspace(f: u32) -> u32 {
    (f >> 16) & 31
}
pub const fn t_channels(f: u32) -> u32 {
    (f >> 3) & 15
}
pub const fn t_bytes(f: u32) -> u32 {
    f & 7
}

pub const TYPE_RGB_8: u32 = colorspace_sh(PT_RGB) | channels_sh(3) | bytes_sh(1);
pub const TYPE_RGB_16: u32 = colorspace_sh(PT_RGB) | channels_sh(3) | bytes_sh(2);
pub const TYPE_LAB_DBL: u32 = float_sh(1) | colorspace_sh(PT_LAB) | channels_sh(3) | bytes_sh(0);
pub const TYPE_XYZ_DBL: u32 = float_sh(1) | colorspace_sh(PT_XYZ) | channels_sh(3) | bytes_sh(0);

pub fn formatter_is_float(f: u32) -> bool {
    t_float(f) != 0
}
pub fn formatter_is_8bit(f: u32) -> bool {
    t_bytes(f) == 1
}

pub fn formatter_for_color_space(cs: u32, bytes: u32, is_float: bool) -> u32 {
    let bits = lcms_color_space(cs);
    let n = channels_of_color_space(cs);
    if n < 0 {
        return 0;
    }
    float_sh(u32::from(is_float))
        | colorspace_sh(bits)
        | bytes_sh(bytes & 7)
        | channels_sh(n as u32)
}

// Fixed point helpers
pub fn quick_floor(val: f64) -> i32 {
    let temp = val + 68719476736.0 * 1.5;
    (temp.to_bits() as u32 as i32) >> 16
}

pub fn quick_floor_word(d: f64) -> u16 {
    (quick_floor(d - 32767.0) as u16).wrapping_add(32767)
}

pub fn quick_saturate_word(d: f64) -> u16 {
    let d = d + 0.5;
    if d <= 0.0 {
        return 0;
    }
    if d >= 65535.0 {
        return 0xffff;
    }
    quick_floor_word(d)
}

pub fn to_fixed_domain(a: i32) -> i32 {
    a.wrapping_add(a.wrapping_add(0x7fff) / 0xffff)
}

pub fn from_8_to_16(b: u8) -> u16 {
    ((b as u16) << 8) | b as u16
}

pub fn from_16_to_8(w: u16) -> u8 {
    (((w as u32) * 65281 + 8388608) >> 24) as u8
}

pub fn quantize_val(i: f64, max_samples: u32) -> u16 {
    let x = (i * 65535.0) / (max_samples.wrapping_sub(1)) as f64;
    quick_saturate_word(x)
}

// CIE structures
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CieXyz {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CieXyY {
    pub x: f64,
    pub y: f64,
    pub big_y: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CieLab {
    pub l: f64,
    pub a: f64,
    pub b: f64,
}

pub type Vec3 = [f64; 3];
pub type Mat3 = [[f64; 3]; 3];

pub fn mat3_identity() -> Mat3 {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

pub fn mat3_is_identity(a: &Mat3) -> bool {
    let id = mat3_identity();
    for i in 0..3 {
        for j in 0..3 {
            if (id[i][j] - a[i][j]).abs() >= 1.0 / 65535.0 {
                return false;
            }
        }
    }
    true
}

pub fn mat3_per(a: &Mat3, b: &Mat3) -> Mat3 {
    let rc = |i: usize, j: usize| a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
    [
        [rc(0, 0), rc(0, 1), rc(0, 2)],
        [rc(1, 0), rc(1, 1), rc(1, 2)],
        [rc(2, 0), rc(2, 1), rc(2, 2)],
    ]
}

pub fn mat3_inverse(a: &Mat3) -> Option<Mat3> {
    let c0 = a[1][1] * a[2][2] - a[1][2] * a[2][1];
    let c1 = -a[1][0] * a[2][2] + a[1][2] * a[2][0];
    let c2 = a[1][0] * a[2][1] - a[1][1] * a[2][0];
    let det = a[0][0] * c0 + a[0][1] * c1 + a[0][2] * c2;
    if det.abs() < MATRIX_DET_TOLERANCE {
        return None;
    }
    Some([
        [
            c0 / det,
            (a[0][2] * a[2][1] - a[0][1] * a[2][2]) / det,
            (a[0][1] * a[1][2] - a[0][2] * a[1][1]) / det,
        ],
        [
            c1 / det,
            (a[0][0] * a[2][2] - a[0][2] * a[2][0]) / det,
            (a[0][2] * a[1][0] - a[0][0] * a[1][2]) / det,
        ],
        [
            c2 / det,
            (a[0][1] * a[2][0] - a[0][0] * a[2][1]) / det,
            (a[0][0] * a[1][1] - a[0][1] * a[1][0]) / det,
        ],
    ])
}

pub fn mat3_eval(a: &Mat3, v: &Vec3) -> Vec3 {
    [
        a[0][0] * v[0] + a[0][1] * v[1] + a[0][2] * v[2],
        a[1][0] * v[0] + a[1][1] * v[1] + a[1][2] * v[2],
        a[2][0] * v[0] + a[2][1] * v[1] + a[2][2] * v[2],
    ]
}

pub fn mat3_flat(m: &Mat3) -> [f64; 9] {
    [
        m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2], m[2][0], m[2][1], m[2][2],
    ]
}

pub fn mat3_from_flat(m: &[f64]) -> Mat3 {
    [[m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]]]
}

// PCS conversions
pub fn xyz2xyy(s: &CieXyz) -> CieXyY {
    let isum = 1.0 / (s.x + s.y + s.z);
    CieXyY {
        x: s.x * isum,
        y: s.y * isum,
        big_y: s.y,
    }
}

pub fn xyy2xyz(s: &CieXyY) -> CieXyz {
    CieXyz {
        x: (s.x / s.y) * s.big_y,
        y: s.big_y,
        z: ((1.0 - s.x - s.y) / s.y) * s.big_y,
    }
}

fn lab_f(t: f64) -> f64 {
    let limit = (24.0 / 116.0) * (24.0 / 116.0) * (24.0 / 116.0);
    if t <= limit {
        (841.0 / 108.0) * t + (16.0 / 116.0)
    } else {
        t.powf(1.0 / 3.0)
    }
}

fn lab_f_1(t: f64) -> f64 {
    let limit = 24.0 / 116.0;
    if t <= limit {
        return (108.0 / 841.0) * (t - (16.0 / 116.0));
    }
    t * t * t
}

pub fn xyz2lab(white: Option<&CieXyz>, xyz: &CieXyz) -> CieLab {
    let w = white.unwrap_or(&D50);
    let fx = lab_f(xyz.x / w.x);
    let fy = lab_f(xyz.y / w.y);
    let fz = lab_f(xyz.z / w.z);
    CieLab {
        l: 116.0 * fy - 16.0,
        a: 500.0 * (fx - fy),
        b: 200.0 * (fy - fz),
    }
}

pub fn lab2xyz(white: Option<&CieXyz>, lab: &CieLab) -> CieXyz {
    let w = white.unwrap_or(&D50);
    let y = (lab.l + 16.0) / 116.0;
    let x = y + 0.002 * lab.a;
    let z = y - 0.005 * lab.b;
    CieXyz {
        x: lab_f_1(x) * w.x,
        y: lab_f_1(y) * w.y,
        z: lab_f_1(z) * w.z,
    }
}

pub fn delta_e(l1: &CieLab, l2: &CieLab) -> f64 {
    let dl = (l1.l - l2.l).abs();
    let da = (l1.a - l2.a).abs();
    let db = (l1.b - l2.b).abs();
    (dl * dl + da * da + db * db).powf(0.5)
}

pub fn d50_xyy() -> CieXyY {
    xyz2xyy(&D50)
}

fn compute_chromatic_adaptation(src: &CieXyz, dst: &CieXyz, chad: &Mat3) -> Option<Mat3> {
    let chad_inv = mat3_inverse(chad)?;
    let cone_src = mat3_eval(chad, &[src.x, src.y, src.z]);
    let cone_dst = mat3_eval(chad, &[dst.x, dst.y, dst.z]);
    if cone_src[0].abs() < MATRIX_DET_TOLERANCE
        || cone_src[1].abs() < MATRIX_DET_TOLERANCE
        || cone_src[2].abs() < MATRIX_DET_TOLERANCE
    {
        return None;
    }
    let cone = [
        [cone_dst[0] / cone_src[0], 0.0, 0.0],
        [0.0, cone_dst[1] / cone_src[1], 0.0],
        [0.0, 0.0, cone_dst[2] / cone_src[2]],
    ];
    let tmp = mat3_per(&cone, chad);
    Some(mat3_per(&chad_inv, &tmp))
}

/// Bradford adaptation matrix from one illuminant to another.
pub fn adaptation_matrix(from: &CieXyz, to: &CieXyz) -> Option<Mat3> {
    let bradford = [
        [0.8951, 0.2664, -0.1614],
        [-0.7502, 1.7135, 0.0367],
        [0.0389, -0.0685, 1.0296],
    ];
    compute_chromatic_adaptation(from, to, &bradford)
}

pub fn build_rgb2xyz_transfer_matrix(white: &CieXyY, prim: &[CieXyY; 3]) -> Option<Mat3> {
    let (xn, yn) = (white.x, white.y);
    let (xr, yr) = (prim[0].x, prim[0].y);
    let (xg, yg) = (prim[1].x, prim[1].y);
    let (xb, yb) = (prim[2].x, prim[2].y);
    let primaries = [
        [xr, xg, xb],
        [yr, yg, yb],
        [(1.0 - xr - yr), (1.0 - xg - yg), (1.0 - xb - yb)],
    ];
    let result = mat3_inverse(&primaries)?;
    let white_point = [xn / yn, 1.0, (1.0 - xn - yn) / yn];
    let coef = mat3_eval(&result, &white_point);
    let r = [
        [coef[0] * xr, coef[1] * xg, coef[2] * xb],
        [coef[0] * yr, coef[1] * yg, coef[2] * yb],
        [
            coef[0] * (1.0 - xr - yr),
            coef[1] * (1.0 - xg - yg),
            coef[2] * (1.0 - xb - yb),
        ],
    ];
    let dn = xyy2xyz(white);
    let bradford = adaptation_matrix(&dn, &D50)?;
    Some(mat3_per(&bradford, &r))
}
