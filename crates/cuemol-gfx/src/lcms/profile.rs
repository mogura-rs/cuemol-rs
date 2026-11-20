//! ICC profile container and tag type readers (port of LittleCMS 2.19.1 cmsio0.c,
//! cmstypes.c and cmsnamed.c, MIT).

use super::base::*;
use super::gamma::ToneCurve;
use super::lut::{Pipeline, Stage, StageData};
use std::cell::OnceCell;
use std::rc::Rc;

pub const SIG_A2B0: u32 = sig(b"A2B0");
pub const SIG_A2B1: u32 = sig(b"A2B1");
pub const SIG_A2B2: u32 = sig(b"A2B2");
pub const SIG_B2A0: u32 = sig(b"B2A0");
pub const SIG_B2A1: u32 = sig(b"B2A1");
pub const SIG_B2A2: u32 = sig(b"B2A2");
pub const SIG_D2B0: u32 = sig(b"D2B0");
pub const SIG_D2B1: u32 = sig(b"D2B1");
pub const SIG_D2B2: u32 = sig(b"D2B2");
pub const SIG_D2B3: u32 = sig(b"D2B3");
pub const SIG_B2D0: u32 = sig(b"B2D0");
pub const SIG_B2D1: u32 = sig(b"B2D1");
pub const SIG_B2D2: u32 = sig(b"B2D2");
pub const SIG_B2D3: u32 = sig(b"B2D3");
pub const SIG_RED_COLORANT: u32 = sig(b"rXYZ");
pub const SIG_GREEN_COLORANT: u32 = sig(b"gXYZ");
pub const SIG_BLUE_COLORANT: u32 = sig(b"bXYZ");
pub const SIG_RED_TRC: u32 = sig(b"rTRC");
pub const SIG_GREEN_TRC: u32 = sig(b"gTRC");
pub const SIG_BLUE_TRC: u32 = sig(b"bTRC");
pub const SIG_GRAY_TRC: u32 = sig(b"kTRC");
pub const SIG_MEDIA_WHITE_POINT: u32 = sig(b"wtpt");
pub const SIG_MEDIA_BLACK_POINT: u32 = sig(b"bkpt");
pub const SIG_CHROMATIC_ADAPTATION: u32 = sig(b"chad");
pub const SIG_PROFILE_DESCRIPTION: u32 = sig(b"desc");
pub const SIG_PROFILE_DESCRIPTION_ML: u32 = sig(b"dscm");
pub const SIG_DEVICE_MFG_DESC: u32 = sig(b"dmnd");
pub const SIG_DEVICE_MODEL_DESC: u32 = sig(b"dmdd");
pub const SIG_COPYRIGHT: u32 = sig(b"cprt");
pub const SIG_NAMED_COLOR2: u32 = sig(b"ncl2");

pub const TYPE_CURVE: u32 = sig(b"curv");
pub const TYPE_PARAMETRIC_CURVE: u32 = sig(b"para");
pub const TYPE_XYZ: u32 = sig(b"XYZ ");
pub const TYPE_S15FIXED16_ARRAY: u32 = sig(b"sf32");
pub const TYPE_TEXT: u32 = sig(b"text");
pub const TYPE_TEXT_DESCRIPTION: u32 = sig(b"desc");
pub const TYPE_MLU: u32 = sig(b"mluc");
pub const TYPE_LUT8: u32 = sig(b"mft1");
pub const TYPE_LUT16: u32 = sig(b"mft2");
pub const TYPE_LUT_A2B: u32 = sig(b"mAB ");
pub const TYPE_LUT_B2A: u32 = sig(b"mBA ");
pub const TYPE_MPE: u32 = sig(b"mpet");
pub const TYPE_NAMED_COLOR2: u32 = sig(b"ncl2");
pub const TYPE_CORBIS_BROKEN_XYZ: u32 = 0x17A5_05B8;
pub const TYPE_MONACO_BROKEN_CURVE: u32 = 0x9478_ee00;

const MAX_TABLE_TAG: u32 = 100;
const MAGIC_NUMBER: u32 = 0x6163_7370;

/// Supported types and minimum element count of the tags this port reads.
fn tag_descriptor(s: u32) -> Option<(u32, &'static [u32])> {
    const LUT_A2B: &[u32] = &[TYPE_LUT16, TYPE_LUT_A2B, TYPE_LUT8];
    const LUT_B2A: &[u32] = &[TYPE_LUT16, TYPE_LUT_B2A, TYPE_LUT8];
    const XYZ: &[u32] = &[TYPE_XYZ, TYPE_CORBIS_BROKEN_XYZ];
    const TRC: &[u32] = &[TYPE_CURVE, TYPE_PARAMETRIC_CURVE, TYPE_MONACO_BROKEN_CURVE];
    const GRAY_TRC: &[u32] = &[TYPE_CURVE, TYPE_PARAMETRIC_CURVE];
    const TEXT: &[u32] = &[TYPE_TEXT, TYPE_MLU, TYPE_TEXT_DESCRIPTION];
    const TEXT_DESC: &[u32] = &[TYPE_TEXT_DESCRIPTION, TYPE_MLU, TYPE_TEXT];
    match s {
        SIG_A2B0 | SIG_A2B1 | SIG_A2B2 => Some((1, LUT_A2B)),
        SIG_B2A0 | SIG_B2A1 | SIG_B2A2 => Some((1, LUT_B2A)),
        SIG_RED_COLORANT | SIG_GREEN_COLORANT | SIG_BLUE_COLORANT => Some((1, XYZ)),
        SIG_MEDIA_WHITE_POINT | SIG_MEDIA_BLACK_POINT => Some((1, XYZ)),
        SIG_RED_TRC | SIG_GREEN_TRC | SIG_BLUE_TRC => Some((1, TRC)),
        SIG_GRAY_TRC => Some((1, GRAY_TRC)),
        SIG_CHROMATIC_ADAPTATION => Some((9, &[TYPE_S15FIXED16_ARRAY])),
        SIG_COPYRIGHT => Some((1, TEXT)),
        SIG_DEVICE_MFG_DESC | SIG_DEVICE_MODEL_DESC | SIG_PROFILE_DESCRIPTION => {
            Some((1, TEXT_DESC))
        }
        SIG_PROFILE_DESCRIPTION_ML => Some((1, &[TYPE_MLU])),
        SIG_D2B0 | SIG_D2B1 | SIG_D2B2 | SIG_D2B3 | SIG_B2D0 | SIG_B2D1 | SIG_B2D2 | SIG_B2D3 => {
            Some((1, &[TYPE_MPE]))
        }
        SIG_NAMED_COLOR2 => Some((1, &[TYPE_NAMED_COLOR2])),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct MluEntry {
    pub language: u16,
    pub country: u16,
    /// Byte offset of the string in a wchar_t (4 byte) pool.
    pub str_w: u32,
    /// Byte length in the wchar_t pool.
    pub len: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Mlu {
    pub entries: Vec<MluEntry>,
    pub pool: Option<Vec<i32>>,
}

impl Mlu {
    fn add_block(&mut self, block: &[i32], language: u16, country: u16) -> bool {
        if self
            .entries
            .iter()
            .any(|e| e.language == language && e.country == country)
        {
            return false;
        }
        let pool = self.pool.get_or_insert_with(Vec::new);
        let offset = (pool.len() * 4) as u32;
        pool.extend_from_slice(block);
        self.entries.push(MluEntry {
            language,
            country,
            str_w: offset,
            len: (block.len() * 4) as u32,
        });
        true
    }

    pub fn set_ascii(&mut self, language: u16, country: u16, text: &[u8]) -> bool {
        let len = text.iter().position(|&b| b == 0).unwrap_or(text.len());
        if len == 0 {
            return self.add_block(&[0], language, country);
        }
        let wide: Vec<i32> = text[..len].iter().map(|&b| b as i8 as i32).collect();
        self.add_block(&wide, language, country)
    }

    pub fn set_wide(&mut self, language: u16, country: u16, wide: &[i32]) -> bool {
        let len = wide.iter().position(|&w| w == 0).unwrap_or(wide.len());
        if len == 0 {
            return self.add_block(&[0], language, country);
        }
        self.add_block(&wide[..len], language, country)
    }

    fn get_wide(&self, language: u16, country: u16) -> Option<(&[i32], u32)> {
        let pool = self.pool.as_deref();
        let slice = |e: &MluEntry| -> Option<(&[i32], u32)> {
            let pool = pool?;
            let start = (e.str_w / 4) as usize;
            Some((pool.get(start..).unwrap_or(&[]), e.len))
        };
        let mut best: Option<usize> = None;
        for (i, e) in self.entries.iter().enumerate() {
            if e.language == language {
                if best.is_none() {
                    best = Some(i);
                }
                if e.country == country {
                    return slice(e);
                }
            }
        }
        let zero = MluEntry {
            language: 0,
            country: 0,
            str_w: 0,
            len: 0,
        };
        let e = self.entries.get(best.unwrap_or(0)).unwrap_or(&zero);
        let pool_size = pool.map_or(0, |p| p.len() * 4) as u64;
        if e.str_w as u64 + e.len as u64 > pool_size {
            return None;
        }
        slice(e)
    }

    /// Bytes produced by cmsMLUgetASCII, without the terminator.
    pub fn get_ascii(&self, language: &[u8; 2], country: &[u8; 2]) -> Option<Vec<u8>> {
        let lang = u16::from_be_bytes(*language);
        let cntry = u16::from_be_bytes(*country);
        let (wide, len) = self.get_wide(lang, cntry)?;
        let n = (len / 4) as usize;
        Some(
            (0..n)
                .map(|i| {
                    let wc = wide.get(i).copied().unwrap_or(0);
                    if wc < 0xff {
                        wc as u8
                    } else {
                        b'?'
                    }
                })
                .collect(),
        )
    }
}

#[derive(Clone, Debug)]
pub enum TagValue {
    Xyz(CieXyz),
    Curve(ToneCurve),
    Pipeline(Pipeline),
    Mlu(Mlu),
    Doubles(Vec<f64>),
}

struct Tag {
    sig: u32,
    offset: u32,
    size: u32,
    value: OnceCell<Option<(u32, Rc<TagValue>)>>,
}

pub struct Profile {
    pub version: u32,
    pub device_class: u32,
    pub color_space: u32,
    pub pcs: u32,
    pub rendering_intent: u32,
    data: Option<Rc<[u8]>>,
    tags: Vec<Tag>,
}

/// Memory reader with the semantics of the LittleCMS memory IO handler.
pub struct Io<'a> {
    data: &'a [u8],
    pos: u32,
}

impl<'a> Io<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Io { data, pos: 0 }
    }

    pub fn substream(&self, offset: u32, size: u32) -> Option<Io<'a>> {
        Some(Io::new(
            self.data
                .get(offset as usize..offset.checked_add(size)? as usize)?,
        ))
    }

    fn size(&self) -> u32 {
        self.data.len() as u32
    }

    pub fn tell(&self) -> u32 {
        self.pos
    }

    pub fn remaining(&self) -> u32 {
        self.size().saturating_sub(self.pos)
    }

    pub fn seek(&mut self, offset: u32) -> bool {
        if offset > self.size() {
            return false;
        }
        self.pos = offset;
        true
    }

    pub fn read(&mut self, len: u32) -> Option<&'a [u8]> {
        if len == 0 {
            return None;
        }
        let size = self.size();
        if len > size || self.pos > size - len {
            return None;
        }
        let start = self.pos as usize;
        self.pos += len;
        Some(&self.data[start..start + len as usize])
    }

    pub fn u8(&mut self) -> Option<u8> {
        self.read(1).map(|b| b[0])
    }

    pub fn u16(&mut self) -> Option<u16> {
        self.read(2).map(|b| u16::from_be_bytes([b[0], b[1]]))
    }

    pub fn u32(&mut self) -> Option<u32> {
        self.read(4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn s15f16(&mut self) -> Option<f64> {
        self.u32().map(|v| v as i32 as f64 / 65536.0)
    }

    pub fn u16_array(&mut self, n: u32) -> Option<Vec<u16>> {
        if n as u64 * 2 > self.remaining() as u64 {
            return None;
        }
        (0..n).map(|_| self.u16()).collect()
    }

    pub fn type_base(&mut self) -> u32 {
        self.read(8)
            .map_or(0, |b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn alignment(&mut self) -> bool {
        let at = self.pos;
        let next = (at.wrapping_add(3)) & !3;
        let n = next.wrapping_sub(at);
        if n == 0 {
            return true;
        }
        if n > 4 {
            return false;
        }
        self.read(n).is_some()
    }
}

fn validated_version(v: u32) -> u32 {
    let mut b = v.to_be_bytes();
    if b[0] > 0x09 {
        b[0] = 0x09;
    }
    let mut t1 = b[1] & 0xf0;
    let mut t2 = b[1] & 0x0f;
    if t1 > 0x90 {
        t1 = 0x90;
    }
    if t2 > 0x09 {
        t2 = 0x09;
    }
    b[1] = t1 | t2;
    b[2] = 0;
    b[3] = 0;
    u32::from_be_bytes(b)
}

fn valid_device_class(cl: u32) -> bool {
    matches!(
        cl,
        0 | SIG_INPUT_CLASS
            | SIG_DISPLAY_CLASS
            | SIG_OUTPUT_CLASS
            | SIG_LINK_CLASS
            | SIG_ABSTRACT_CLASS
            | SIG_COLORSPACE_CLASS
            | SIG_NAMED_COLOR_CLASS
            | SIG_COLOR_ENCODING_CLASS
            | SIG_MULTIPLEX_ID_CLASS
            | SIG_MULTIPLEX_LINK_CLASS
            | SIG_MULTIPLEX_VIS_CLASS
    )
}

impl Profile {
    pub fn placeholder() -> Profile {
        Profile {
            version: 0x0210_0000,
            device_class: SIG_DISPLAY_CLASS,
            color_space: 0,
            pcs: 0,
            rendering_intent: 0,
            data: None,
            tags: Vec::new(),
        }
    }

    pub fn open_mem(bytes: &[u8]) -> Option<Profile> {
        if bytes.len() > u32::MAX as usize {
            return None;
        }
        let data: Rc<[u8]> = Rc::from(bytes);
        let mut io = Io::new(&data);
        let header = io.read(128)?;
        let be =
            |o: usize| u32::from_be_bytes([header[o], header[o + 1], header[o + 2], header[o + 3]]);
        if be(36) != MAGIC_NUMBER {
            return None;
        }
        let mut p = Profile::placeholder();
        p.device_class = be(12);
        p.color_space = be(16);
        p.pcs = be(20);
        p.rendering_intent = be(64);
        p.version = validated_version(be(8));
        if p.version > 0x0500_0000 {
            return None;
        }
        if !valid_device_class(p.device_class) {
            return None;
        }
        let reported = bytes.len() as u32;
        let mut header_size = be(0);
        if header_size >= reported {
            header_size = reported;
        }
        let count = io.u32()?;
        if count > MAX_TABLE_TAG {
            return None;
        }
        let mut tags = Vec::new();
        for _ in 0..count {
            let s = io.u32()?;
            let offset = io.u32()?;
            let size = io.u32()?;
            if size == 0 || offset == 0 {
                continue;
            }
            let end = offset.wrapping_add(size);
            if end > header_size || end < offset {
                continue;
            }
            tags.push(Tag {
                sig: s,
                offset,
                size,
                value: OnceCell::new(),
            });
        }
        for (i, a) in tags.iter().enumerate() {
            for (j, b) in tags.iter().enumerate() {
                if i != j && a.sig == b.sig {
                    return None;
                }
            }
        }
        p.tags = tags;
        p.data = Some(data.clone());
        Some(p)
    }

    pub fn set_version(&mut self, version: f64) {
        let mut v = (version * 100.0 + 0.5).floor() as u32;
        let mut digits = Vec::new();
        while v > 0 {
            digits.push(v % 10);
            v /= 10;
        }
        let mut out = 0u32;
        for d in digits.iter().rev() {
            out = out.wrapping_mul(16).wrapping_add(*d);
        }
        self.version = out << 16;
    }

    pub fn profile_version(&self) -> f64 {
        let mut n = self.version >> 16;
        let mut digits = Vec::new();
        while n > 0 {
            digits.push(n % 16);
            n /= 16;
        }
        let mut out = 0u32;
        for d in digits.iter().rev() {
            out = out.wrapping_mul(10).wrapping_add(*d);
        }
        out as f64 / 100.0
    }

    pub fn is_tag(&self, s: u32) -> bool {
        self.tags.iter().any(|t| t.sig == s)
    }

    pub fn write_tag(&mut self, s: u32, typ: u32, value: TagValue) {
        let cell = OnceCell::new();
        let _ = cell.set(Some((typ, Rc::new(value))));
        let tag = Tag {
            sig: s,
            offset: 0,
            size: 0,
            value: cell,
        };
        if let Some(t) = self.tags.iter_mut().find(|t| t.sig == s) {
            *t = tag;
        } else {
            self.tags.push(tag);
        }
    }

    /// Reads (and caches) a tag. Returns the original type and the value.
    pub fn read_tag_typed(&self, s: u32) -> Option<(u32, Rc<TagValue>)> {
        let tag = self.tags.iter().find(|t| t.sig == s)?;
        let (elem_count, supported) = tag_descriptor(s)?;
        let entry = tag
            .value
            .get_or_init(|| self.load_tag(tag, elem_count, supported));
        let (typ, value) = entry.as_ref()?;
        if !supported.contains(typ) {
            return None;
        }
        Some((*typ, value.clone()))
    }

    pub fn read_tag(&self, s: u32) -> Option<Rc<TagValue>> {
        self.read_tag_typed(s).map(|(_, v)| v)
    }

    fn load_tag(
        &self,
        tag: &Tag,
        elem_count: u32,
        supported: &[u32],
    ) -> Option<(u32, Rc<TagValue>)> {
        let data = self.data.as_ref()?;
        if tag.size < 8 {
            return None;
        }
        let mut io =
            Io::new(data.get(tag.offset as usize..tag.offset.checked_add(tag.size)? as usize)?);
        let base = io.type_base();
        if base == 0 || !supported.contains(&base) {
            return None;
        }
        let size = tag.size - 8;
        let (value, n_items) = read_type(base, &mut io, size, self.version)?;
        if n_items < elem_count {
            return None;
        }
        Some((base, Rc::new(value)))
    }

    pub fn read_xyz(&self, s: u32) -> Option<CieXyz> {
        match self.read_tag(s)?.as_ref() {
            TagValue::Xyz(x) => Some(*x),
            _ => None,
        }
    }

    pub fn read_curve(&self, s: u32) -> Option<Rc<TagValue>> {
        let v = self.read_tag(s)?;
        matches!(v.as_ref(), TagValue::Curve(_)).then_some(v)
    }
}

pub fn as_curve(v: &TagValue) -> &ToneCurve {
    match v {
        TagValue::Curve(c) => c,
        _ => unreachable!("tag value is not a curve"),
    }
}

fn read_type(base: u32, io: &mut Io, size: u32, _version: u32) -> Option<(TagValue, u32)> {
    match base {
        TYPE_XYZ | TYPE_CORBIS_BROKEN_XYZ => {
            let x = io.s15f16()?;
            let y = io.s15f16()?;
            let z = io.s15f16()?;
            Some((TagValue::Xyz(CieXyz { x, y, z }), 1))
        }
        TYPE_CURVE | TYPE_MONACO_BROKEN_CURVE => Some((TagValue::Curve(read_curve(io)?), 1)),
        TYPE_PARAMETRIC_CURVE => Some((TagValue::Curve(read_parametric(io)?), 1)),
        TYPE_S15FIXED16_ARRAY => {
            let n = size / 4;
            if n as u64 * 4 > io.remaining() as u64 {
                return None;
            }
            let v = (0..n).map(|_| io.s15f16()).collect::<Option<Vec<_>>>()?;
            Some((TagValue::Doubles(v), n))
        }
        TYPE_TEXT => Some((TagValue::Mlu(read_text(io, size)?), 1)),
        TYPE_TEXT_DESCRIPTION => Some((TagValue::Mlu(read_text_description(io, size)?), 1)),
        TYPE_MLU => Some((TagValue::Mlu(read_mlu(io, size)?), 1)),
        TYPE_MPE => Some((TagValue::Pipeline(super::mpe::read(io)?), 1)),
        TYPE_LUT8 => Some((TagValue::Pipeline(read_lut8(io)?), 1)),
        TYPE_LUT16 => Some((TagValue::Pipeline(read_lut16(io)?), 1)),
        TYPE_LUT_A2B => Some((TagValue::Pipeline(read_lut_a2b(io)?), 1)),
        TYPE_LUT_B2A => Some((TagValue::Pipeline(read_lut_b2a(io)?), 1)),
        _ => None,
    }
}

fn read_curve(io: &mut Io) -> Option<ToneCurve> {
    let count = io.u32()?;
    match count {
        0 => ToneCurve::parametric(1, &[1.0]),
        1 => {
            let fixed = io.u16()?;
            let gamma = (fixed >> 8) as f64 + ((fixed & 0xff) as f64 / 256.0);
            ToneCurve::parametric(1, &[gamma])
        }
        _ => {
            if count > 0x7FFF {
                return None;
            }
            let table = io.u16_array(count)?;
            ToneCurve::tabulated16(count, Some(&table))
        }
    }
}

fn read_parametric(io: &mut Io) -> Option<ToneCurve> {
    const PARAMS_BY_TYPE: [usize; 5] = [1, 3, 4, 5, 7];
    let typ = io.u16()?;
    io.u16()?;
    if typ > 4 {
        return None;
    }
    let mut params = [0.0f64; 10];
    for p in params.iter_mut().take(PARAMS_BY_TYPE[typ as usize]) {
        *p = io.s15f16()?;
    }
    ToneCurve::parametric(typ as i32 + 1, &params)
}

fn read_text(io: &mut Io, size: u32) -> Option<Mlu> {
    if size == u32::MAX || size > io.remaining() {
        return None;
    }
    let text = if size == 0 { &[][..] } else { io.read(size)? };
    let mut mlu = Mlu::default();
    if !mlu.set_ascii(0, 0, text) {
        return None;
    }
    Some(mlu)
}

/// Mirrors convert_utf16_to_utf32: consumes UTF-16 units, combining surrogate pairs.
fn read_utf16_as_wide(io: &mut Io, n: u32) -> Option<Vec<i32>> {
    let mut out = Vec::new();
    let mut n = n as i64;
    while n > 0 {
        let uc = io.u16()? as u32;
        n -= 1;
        if uc.wrapping_sub(0xd800) >= 2048 {
            out.push(uc as i32);
        } else {
            let low = io.u16()? as u32;
            n -= 1;
            if (uc & 0xfffffc00) == 0xd800 && (low & 0xfffffc00) == 0xdc00 {
                out.push(((uc << 10).wrapping_add(low).wrapping_sub(0x35fdc00)) as i32);
            } else {
                return None;
            }
        }
    }
    Some(out)
}

fn read_text_description(io: &mut Io, size: u32) -> Option<Mlu> {
    let mut size = size;
    if size < 4 {
        return None;
    }
    let ascii_count = io.u32()?;
    if ascii_count > 0x7ffff {
        return None;
    }
    size -= 4;
    if size < ascii_count {
        return None;
    }
    let mut mlu = Mlu::default();
    let text = if ascii_count == 0 {
        &[][..]
    } else {
        io.read(ascii_count)?
    };
    size -= ascii_count;
    if !mlu.set_ascii(0, 0, text) {
        return None;
    }
    'done: {
        if size < 8 {
            break 'done;
        }
        let Some(_unicode_code) = io.u32() else {
            break 'done;
        };
        let Some(unicode_count) = io.u32() else {
            break 'done;
        };
        size -= 8;
        if unicode_count == 0 || unicode_count > 0x7ffff || (size as u64) < unicode_count as u64 * 2
        {
            break 'done;
        }
        let Some(mut wide) = read_utf16_as_wide(io, unicode_count) else {
            break 'done;
        };
        wide.resize(unicode_count as usize + 1, 0);
        if !mlu.set_wide(0xffff, 0xffff, &wide) {
            break 'done;
        }
        size -= unicode_count * 2;
        if size >= 2 + 1 + 67 {
            if io.u16().is_none() || io.u8().is_none() {
                break 'done;
            }
            for _ in 0..67 {
                io.u8()?;
            }
        }
    }
    Some(mlu)
}

fn read_mlu(io: &mut Io, size: u32) -> Option<Mlu> {
    let count = io.u32()?;
    let rec_len = io.u32()?;
    if rec_len != 12 {
        return None;
    }
    if count as u64 * 12 > io.remaining() as u64 {
        return None;
    }
    let size_of_header = 12u32.wrapping_mul(count).wrapping_add(8);
    let mut largest = 0u32;
    let mut entries = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let language = io.u16()?;
        let country = io.u16()?;
        let len = io.u32()?;
        let offset = io.u32()?;
        if offset & 1 != 0 {
            return None;
        }
        if offset < size_of_header.wrapping_add(8) {
            return None;
        }
        let end = offset.wrapping_add(len);
        if end < len || end > size.wrapping_add(8) {
            return None;
        }
        let begin = offset - size_of_header - 8;
        entries.push(MluEntry {
            language,
            country,
            len: ((len as u64 * 4) / 2) as u32,
            str_w: ((begin as u64 * 4) / 2) as u32,
        });
        let end_of_string = begin.wrapping_add(len);
        if end_of_string > largest {
            largest = end_of_string;
        }
    }
    let pool_bytes = ((largest as u64 * 4) / 2) as u32;
    let pool = if pool_bytes == 0 {
        None
    } else {
        if pool_bytes & 1 != 0 {
            return None;
        }
        let n_wchar = pool_bytes / 4;
        if n_wchar as u64 * 2 > io.remaining() as u64 {
            return None;
        }
        let mut block = read_utf16_as_wide(io, n_wchar)?;
        block.resize((pool_bytes / 4) as usize, 0);
        Some(block)
    };
    Some(Mlu { entries, pool })
}

fn uipow(n: u32, a: u32, b: u32) -> u32 {
    let mut rv: u32 = 1;
    if a == 0 || n == 0 {
        return 0;
    }
    for _ in 0..b {
        rv = rv.wrapping_mul(a);
        if rv > u32::MAX / a {
            return u32::MAX;
        }
    }
    let rc = rv.wrapping_mul(n);
    if rv != rc / n {
        return u32::MAX;
    }
    rc
}

fn read_matrix9(io: &mut Io) -> Option<[f64; 9]> {
    let mut m = [0.0; 9];
    for v in &mut m {
        *v = io.s15f16()?;
    }
    Some(m)
}

fn read_8bit_tables(io: &mut Io, lut: &mut Pipeline, n: usize) -> bool {
    if n > MAX_CHANNELS || n == 0 {
        return false;
    }
    let mut tables = Vec::with_capacity(n);
    for _ in 0..n {
        let Some(raw) = io.read(256) else {
            return false;
        };
        let t: Vec<u16> = raw.iter().map(|&b| from_8_to_16(b)).collect();
        let Some(c) = ToneCurve::tabulated16(256, Some(&t)) else {
            return false;
        };
        tables.push(c);
    }
    lut.insert_end(Stage::tone_curves(n, Some(&tables)))
}

fn read_16bit_tables(io: &mut Io, lut: &mut Pipeline, n: usize, entries: u32) -> bool {
    if entries == 0 {
        return true;
    }
    if entries < 2 || n > MAX_CHANNELS {
        return false;
    }
    let mut tables = Vec::with_capacity(n);
    for _ in 0..n {
        let Some(t) = io.u16_array(entries) else {
            return false;
        };
        let Some(c) = ToneCurve::tabulated16(entries, Some(&t)) else {
            return false;
        };
        tables.push(c);
    }
    lut.insert_end(Stage::tone_curves(n, Some(&tables)))
}

fn read_lut8(io: &mut Io) -> Option<Pipeline> {
    let in_ch = io.u8()? as usize;
    let out_ch = io.u8()? as usize;
    let clut_points = io.u8()?;
    if clut_points == 1 {
        return None;
    }
    io.u8()?;
    if in_ch == 0 || in_ch > MAX_CHANNELS || out_ch == 0 || out_ch > MAX_CHANNELS {
        return None;
    }
    let mut lut = Pipeline::new(in_ch, out_ch)?;
    let m = read_matrix9(io)?;
    if in_ch == 3
        && !mat3_is_identity(&mat3_from_flat(&m))
        && !lut.insert_begin(Stage::matrix(3, 3, &m, None))
    {
        return None;
    }
    if !read_8bit_tables(io, &mut lut, in_ch) {
        return None;
    }
    let tab_size = uipow(out_ch as u32, clut_points as u32, in_ch as u32);
    if tab_size == u32::MAX {
        return None;
    }
    if tab_size > 0 {
        if tab_size > io.remaining() {
            return None;
        }
        let raw = io.read(tab_size)?;
        let t: Vec<u16> = raw.iter().map(|&b| from_8_to_16(b)).collect();
        if !lut.insert_end(Stage::clut16(clut_points as u32, in_ch, out_ch, Some(&t))) {
            return None;
        }
    }
    if !read_8bit_tables(io, &mut lut, out_ch) {
        return None;
    }
    Some(lut)
}

fn read_lut16(io: &mut Io) -> Option<Pipeline> {
    let in_ch = io.u8()? as usize;
    let out_ch = io.u8()? as usize;
    let clut_points = io.u8()?;
    io.u8()?;
    if in_ch == 0 || in_ch > MAX_CHANNELS || out_ch == 0 || out_ch > MAX_CHANNELS {
        return None;
    }
    let mut lut = Pipeline::new(in_ch, out_ch)?;
    let m = read_matrix9(io)?;
    if in_ch == 3
        && !mat3_is_identity(&mat3_from_flat(&m))
        && !lut.insert_end(Stage::matrix(3, 3, &m, None))
    {
        return None;
    }
    let in_entries = io.u16()? as u32;
    let out_entries = io.u16()? as u32;
    if in_entries > 0x7FFF || out_entries > 0x7FFF {
        return None;
    }
    if clut_points == 1 {
        return None;
    }
    if !read_16bit_tables(io, &mut lut, in_ch, in_entries) {
        return None;
    }
    let tab_size = uipow(out_ch as u32, clut_points as u32, in_ch as u32);
    if tab_size == u32::MAX {
        return None;
    }
    if tab_size > 0 {
        let t = io.u16_array(tab_size)?;
        if !lut.insert_end(Stage::clut16(clut_points as u32, in_ch, out_ch, Some(&t))) {
            return None;
        }
    }
    if !read_16bit_tables(io, &mut lut, out_ch, out_entries) {
        return None;
    }
    Some(lut)
}

fn read_matrix_stage(io: &mut Io, offset: u32) -> Option<Stage> {
    if !io.seek(offset) {
        return None;
    }
    let m = read_matrix9(io)?;
    let mut off = [0.0; 3];
    for v in &mut off {
        *v = io.s15f16()?;
    }
    Stage::matrix(3, 3, &m, Some(&off))
}

fn read_clut(io: &mut Io, offset: u32, in_ch: usize, out_ch: usize) -> Option<Stage> {
    if !io.seek(offset) {
        return None;
    }
    let grid8 = io.read(MAX_CHANNELS as u32)?;
    let mut grid = [0u32; MAX_INPUT_DIMENSIONS];
    for (i, &g) in grid8.iter().enumerate() {
        if g == 1 {
            return None;
        }
        if i < MAX_INPUT_DIMENSIONS {
            grid[i] = g as u32;
        }
    }
    let precision = io.u8()?;
    io.u8()?;
    io.u8()?;
    io.u8()?;
    if in_ch > grid.len() || !(1..=2).contains(&precision) {
        return None;
    }
    let entries = super::lut::cube_size(&grid, in_ch).checked_mul(out_ch as u32)?;
    if entries.checked_mul(u32::from(precision))? > io.remaining() {
        return None;
    }
    let mut clut = Stage::clut16_granular(&grid, in_ch, out_ch, None)?;
    let StageData::Clut16 { table, .. } = &mut clut.data else {
        return None;
    };
    let n = table.len() as u32;
    match precision {
        1 => {
            if n > io.remaining() {
                return None;
            }
            for v in table.iter_mut() {
                *v = from_8_to_16(io.u8()?);
            }
        }
        2 => {
            *table = io.u16_array(n)?;
        }
        _ => return None,
    }
    Some(clut)
}

fn read_embedded_curve(io: &mut Io) -> Option<ToneCurve> {
    match io.type_base() {
        TYPE_CURVE => read_curve(io),
        TYPE_PARAMETRIC_CURVE => read_parametric(io),
        _ => None,
    }
}

fn read_set_of_curves(io: &mut Io, offset: u32, n: usize) -> Option<Stage> {
    if n > MAX_CHANNELS {
        return None;
    }
    if !io.seek(offset) {
        return None;
    }
    let mut curves = Vec::with_capacity(n);
    for _ in 0..n {
        curves.push(read_embedded_curve(io)?);
        if !io.alignment() {
            return None;
        }
    }
    Stage::tone_curves(n, Some(&curves))
}

fn read_lut_a2b(io: &mut Io) -> Option<Pipeline> {
    let base = io.tell().wrapping_sub(8);
    let in_ch = io.u8()? as usize;
    let out_ch = io.u8()? as usize;
    io.u16()?;
    let off_b = io.u32()?;
    let off_mat = io.u32()?;
    let off_m = io.u32()?;
    let off_c = io.u32()?;
    let off_a = io.u32()?;
    if in_ch == 0 || in_ch >= MAX_CHANNELS || out_ch == 0 || out_ch >= MAX_CHANNELS {
        return None;
    }
    let mut lut = Pipeline::new(in_ch, out_ch)?;
    if off_a != 0 && !lut.insert_end(read_set_of_curves(io, base.wrapping_add(off_a), in_ch)) {
        return None;
    }
    if off_c != 0 && !lut.insert_end(read_clut(io, base.wrapping_add(off_c), in_ch, out_ch)) {
        return None;
    }
    if off_m != 0 && !lut.insert_end(read_set_of_curves(io, base.wrapping_add(off_m), out_ch)) {
        return None;
    }
    if off_mat != 0 && !lut.insert_end(read_matrix_stage(io, base.wrapping_add(off_mat))) {
        return None;
    }
    if off_b != 0 && !lut.insert_end(read_set_of_curves(io, base.wrapping_add(off_b), out_ch)) {
        return None;
    }
    Some(lut)
}

fn read_lut_b2a(io: &mut Io) -> Option<Pipeline> {
    let base = io.tell().wrapping_sub(8);
    let in_ch = io.u8()? as usize;
    let out_ch = io.u8()? as usize;
    if in_ch == 0 || in_ch >= MAX_CHANNELS || out_ch == 0 || out_ch >= MAX_CHANNELS {
        return None;
    }
    io.u16()?;
    let off_b = io.u32()?;
    let off_mat = io.u32()?;
    let off_m = io.u32()?;
    let off_c = io.u32()?;
    let off_a = io.u32()?;
    let mut lut = Pipeline::new(in_ch, out_ch)?;
    if off_b != 0 && !lut.insert_end(read_set_of_curves(io, base.wrapping_add(off_b), in_ch)) {
        return None;
    }
    if off_mat != 0 && !lut.insert_end(read_matrix_stage(io, base.wrapping_add(off_mat))) {
        return None;
    }
    if off_m != 0 && !lut.insert_end(read_set_of_curves(io, base.wrapping_add(off_m), in_ch)) {
        return None;
    }
    if off_c != 0 && !lut.insert_end(read_clut(io, base.wrapping_add(off_c), in_ch, out_ch)) {
        return None;
    }
    if off_a != 0 && !lut.insert_end(read_set_of_curves(io, base.wrapping_add(off_a), out_ch)) {
        return None;
    }
    Some(lut)
}
