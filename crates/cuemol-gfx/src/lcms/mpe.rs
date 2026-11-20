//! ICC floating-point multi-process elements used in DToB and BToD transforms.

use super::base::{sig, MAX_CHANNELS};
use super::gamma::{CurveSegment, ToneCurve};
use super::lut::{Pipeline, Stage};
use super::profile::Io;

fn channels(io: &mut Io) -> Option<(usize, usize)> {
    let input = usize::from(io.u16()?);
    let output = usize::from(io.u16()?);
    ((1..MAX_CHANNELS).contains(&input) && (1..MAX_CHANNELS).contains(&output))
        .then_some((input, output))
}

fn number(io: &mut Io) -> Option<f32> {
    let v = f32::from_bits(io.u32()?);
    ((v == 0.0 || v.is_normal()) && v.abs() <= 1e20).then_some(v)
}

fn floats(io: &mut Io, count: usize) -> Option<Vec<f32>> {
    if count > io.remaining() as usize / 4 {
        return None;
    }
    (0..count).map(|_| number(io)).collect()
}

fn elements<'a>(io: &mut Io<'a>, count: usize) -> Option<Vec<Io<'a>>> {
    if count > io.remaining() as usize / 8 {
        return None;
    }
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let start = io.u32()?;
        let len = io.u32()?;
        entries.push(io.substream(start, len)?);
    }
    Some(entries)
}

fn curve(mut io: Io) -> Option<ToneCurve> {
    if io.type_base() != sig(b"curf") {
        return None;
    }
    let count = usize::from(io.u16()?);
    io.u16()?;
    let mut ends = floats(&mut io, count.checked_sub(1)?)?;
    ends.push(1e22);
    let mut start = -1e22;
    let mut segments = Vec::with_capacity(count);
    for end in ends {
        let kind = io.type_base();
        let mut segment = CurveSegment::parametric(start, end, 0, [0.0; 10]);
        if kind == sig(b"parf") {
            let formula = usize::from(io.u16()?);
            io.u16()?;
            segment.typ = formula as i32 + 6;
            for p in segment.params.iter_mut().take(*[4, 5, 5].get(formula)?) {
                *p = f64::from(number(&mut io)?);
            }
        } else if kind == sig(b"samf") {
            let count = io.u32()? as usize;
            segment.sampled = vec![0.0];
            segment.sampled.extend(floats(&mut io, count)?);
        } else {
            return None;
        }
        segments.push(segment);
        start = end;
    }
    let mut curve = ToneCurve::segmented(segments)?;
    for i in 0..curve.segments.len() {
        if curve.segments[i].typ == 0 {
            let first = curve.eval_float(curve.segments[i].x0);
            curve.segments[i].sampled[0] = first;
        }
    }
    Some(curve)
}

pub fn read(io: &mut Io) -> Option<Pipeline> {
    let (input, output) = channels(io)?;
    let count = io.u32()? as usize;
    let mut lut = Pipeline::new(input, output)?;
    for mut element in elements(io, count)? {
        let kind = element.type_base();
        if kind == sig(b"bACS") || kind == sig(b"eACS") {
            continue;
        }
        let (cols, rows) = channels(&mut element)?;
        let stage = if kind == sig(b"matf") {
            let values: Vec<f64> = floats(&mut element, (cols + 1) * rows)?
                .into_iter()
                .map(f64::from)
                .collect();
            let (matrix, offsets) = values.split_at(cols * rows);
            Stage::matrix(rows, cols, matrix, Some(offsets))
        } else if kind == sig(b"cvst") && cols == rows {
            let curves = elements(&mut element, cols)?
                .into_iter()
                .map(curve)
                .collect::<Option<Vec<_>>>()?;
            Stage::tone_curves(cols, Some(&curves))
        } else if kind == sig(b"clut") {
            let grid = element.read(16)?[..cols]
                .iter()
                .copied()
                .map(u32::from)
                .collect::<Vec<_>>();
            let n = super::lut::cube_size(&grid, cols).checked_mul(rows as u32)?;
            if n == 0 || u64::from(n) * 4 > super::lut::MAX_MEMORY_FOR_ALLOC {
                return None;
            }
            Stage::clut_float(&grid, rows, floats(&mut element, n as usize)?)
        } else {
            return None;
        };
        if !lut.insert_end(stage) {
            return None;
        }
    }
    (lut.input_channels == input && lut.output_channels == output).then_some(lut)
}
