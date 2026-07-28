//! XDR primitives used by GROMACS trajectory formats.

use cuemol_core::{CueMolError, Result};

const MAGIC_INTS: [i32; 73] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 10, 12, 16, 20, 25, 32, 40, 50, 64, 80, 101, 128, 161, 203, 256,
    322, 406, 512, 645, 812, 1024, 1290, 1625, 2048, 2580, 3250, 4096, 5060, 6501, 8192, 10321,
    13003, 16384, 20642, 26007, 32768, 41285, 52015, 65536, 82570, 104031, 131072, 165140, 208063,
    262144, 330280, 416127, 524287, 660561, 832255, 1048576, 1321122, 1664510, 2097152, 2642245,
    3329021, 4194304, 5284491, 6658042, 8388607, 10568983, 13316085, 16777216,
];
const FIRST_MAGIC_INDEX: usize = 9;

/// Big-endian XDR reader over an in-memory byte slice.
#[derive(Debug, Clone)]
pub struct XdrInStream<'a> {
    input: &'a [u8],
    cursor: usize,
}

impl<'a> XdrInStream<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        Self { input, cursor: 0 }
    }

    pub fn position(&self) -> usize {
        self.cursor
    }

    pub fn remaining(&self) -> usize {
        self.input.len().saturating_sub(self.cursor)
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub fn read_i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.read_array()?))
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.read_array()?))
    }

    pub fn read_f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.read_u32()?))
    }

    pub fn read_f64(&mut self) -> Result<f64> {
        Ok(f64::from_be_bytes(self.read_array()?))
    }

    /// Read an int32 at a frame boundary.
    ///
    /// A clean end of input returns `None`; a partial int32 is an error.
    pub fn read_i32_optional(&mut self) -> Result<Option<i32>> {
        if self.is_empty() {
            return Ok(None);
        }
        if self.remaining() < 4 {
            return Err(CueMolError::parse("XDR truncated int32 at frame boundary"));
        }
        self.read_i32().map(Some)
    }

    /// Read variable-length opaque XDR data including four-byte padding.
    pub fn read_opaque(&mut self) -> Result<&'a [u8]> {
        self.read_opaque_with_length(false)
    }

    /// Read opaque XDR data with the optional GROMACS 2023 64-bit count.
    pub fn read_opaque_with_length(&mut self, long_format: bool) -> Result<&'a [u8]> {
        let count = if long_format {
            (u64::from(self.read_u32()?) << 32) | u64::from(self.read_u32()?)
        } else {
            u64::from(self.read_u32()?)
        };
        let count = usize::try_from(count)
            .map_err(|_| CueMolError::parse("XDR opaque block is too large"))?;
        let data = self.read_exact(count)?;
        let padding = (4 - count % 4) % 4;
        self.skip_bytes(padding)?;
        Ok(data)
    }

    /// Read the duplicated-length string representation used by GROMACS.
    pub fn read_gmx_string(&mut self) -> Result<String> {
        let declared = self.read_u32()? as usize;
        let data = self.read_opaque()?;
        if declared != data.len() && declared != data.len().saturating_add(1) {
            return Err(CueMolError::parse(format!(
                "XDR GROMACS string length mismatch {declared} != {}",
                data.len()
            )));
        }
        let data = data.strip_suffix(&[0]).unwrap_or(data);
        String::from_utf8(data.to_vec())
            .map_err(|err| CueMolError::parse(format!("XDR invalid UTF-8 string: {err}")))
    }

    pub fn read_f32_array(&mut self, count: usize) -> Result<Vec<f32>> {
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(self.read_f32()?);
        }
        Ok(values)
    }

    pub fn read_f64_array_as_f32(&mut self, count: usize) -> Result<Vec<f32>> {
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(self.read_f64()? as f32);
        }
        Ok(values)
    }

    /// Read a GROMACS 3x3 box and convert nm/radians to Å/degrees cell values.
    pub fn read_gmx_box(&mut self, double_precision: bool) -> Result<[f64; 6]> {
        let mut matrix = [0.0_f64; 9];
        for value in &mut matrix {
            *value = if double_precision {
                self.read_f64()?
            } else {
                self.read_f32()? as f64
            } * 10.0;
        }

        let a = [matrix[0], matrix[1], matrix[2]];
        let b = [matrix[3], matrix[4], matrix[5]];
        let c = [matrix[6], matrix[7], matrix[8]];
        let alen = norm(a);
        let blen = norm(b);
        let clen = norm(c);
        Ok([
            alen,
            blen,
            clen,
            vector_angle_degrees(b, c),
            vector_angle_degrees(a, c),
            vector_angle_degrees(a, b),
        ])
    }

    /// Read and decompress the coordinate payload of an XTC frame.
    pub fn read_compressed_coordinates(
        &mut self,
        atom_count: usize,
        long_format: bool,
    ) -> Result<(Vec<f32>, f32)> {
        let precision = self.read_f32()?;
        if !precision.is_finite() || precision <= 0.0 {
            return Err(CueMolError::parse("XTC coordinate precision is invalid"));
        }
        let min = [self.read_i32()?, self.read_i32()?, self.read_i32()?];
        let max = [self.read_i32()?, self.read_i32()?, self.read_i32()?];
        let small_index = self.read_u32()? as usize;
        if !(FIRST_MAGIC_INDEX..MAGIC_INTS.len()).contains(&small_index) {
            return Err(CueMolError::parse(
                "XTC internal overflow in small coordinate index",
            ));
        }
        let (sizes, bits_per_axis, combined_bits) = coordinate_sizes(min, max)?;
        let compressed = self.read_opaque_with_length(long_format)?;
        let mut bits = BitReader::new(compressed);
        let mut output = Vec::with_capacity(atom_count.saturating_mul(3));
        let mut encoded_atoms = 0_usize;
        let mut run = 0_usize;
        let mut current_small_index = small_index;
        let mut smaller =
            MAGIC_INTS[current_small_index.saturating_sub(1).max(FIRST_MAGIC_INDEX)] / 2;
        let mut small_number = MAGIC_INTS[current_small_index] / 2;

        while encoded_atoms < atom_count {
            let mut base = if combined_bits == 0 {
                [
                    bits.read_bits(bits_per_axis[0])? as i32,
                    bits.read_bits(bits_per_axis[1])? as i32,
                    bits.read_bits(bits_per_axis[2])? as i32,
                ]
            } else {
                decode_mixed_integers(&mut bits, combined_bits, sizes)?
            };
            for axis in 0..3 {
                base[axis] = base[axis]
                    .checked_add(min[axis])
                    .ok_or_else(|| CueMolError::parse("XTC coordinate overflow"))?;
            }
            encoded_atoms += 1;

            let mut adjustment = 0_i32;
            if bits.read_bits(1)? != 0 {
                run = bits.read_bits(5)? as usize;
                adjustment = (run % 3) as i32 - 1;
                run -= run % 3;
            }
            let extra_atoms = run / 3;
            if encoded_atoms
                .checked_add(extra_atoms)
                .is_none_or(|count| count > atom_count)
            {
                return Err(CueMolError::parse(
                    "XTC coordinate buffer overrun during decompression",
                ));
            }

            let mut previous = base;
            if extra_atoms == 0 {
                append_scaled_coordinate(&mut output, previous, precision);
            } else {
                let small_sizes = [MAGIC_INTS[current_small_index] as u32; 3];
                for extra_index in 0..extra_atoms {
                    let mut current =
                        decode_mixed_integers(&mut bits, current_small_index as u32, small_sizes)?;
                    encoded_atoms += 1;
                    for axis in 0..3 {
                        let delta = previous[axis]
                            .checked_sub(small_number)
                            .ok_or_else(|| CueMolError::parse("XTC coordinate overflow"))?;
                        current[axis] = current[axis]
                            .checked_add(delta)
                            .ok_or_else(|| CueMolError::parse("XTC coordinate overflow"))?;
                    }
                    if extra_index == 0 {
                        std::mem::swap(&mut current, &mut previous);
                        append_scaled_coordinate(&mut output, previous, precision);
                    } else {
                        previous = current;
                    }
                    append_scaled_coordinate(&mut output, current, precision);
                }
            }

            match adjustment {
                -1 => {
                    if current_small_index == FIRST_MAGIC_INDEX {
                        return Err(CueMolError::parse("XTC small coordinate index underflow"));
                    }
                    current_small_index -= 1;
                    small_number = smaller;
                    smaller = if current_small_index > FIRST_MAGIC_INDEX {
                        MAGIC_INTS[current_small_index - 1] / 2
                    } else {
                        0
                    };
                }
                1 => {
                    current_small_index += 1;
                    let Some(&next) = MAGIC_INTS.get(current_small_index) else {
                        return Err(CueMolError::parse("XTC small coordinate index overflow"));
                    };
                    smaller = small_number;
                    small_number = next / 2;
                }
                _ => {}
            }
            if MAGIC_INTS[current_small_index] == 0 {
                return Err(CueMolError::parse("XTC invalid small coordinate size"));
            }
        }

        if output.len() != atom_count.saturating_mul(3) {
            return Err(CueMolError::parse(
                "XTC decompressed coordinate count mismatch",
            ));
        }
        Ok((output, precision))
    }

    pub fn skip_bytes(&mut self, count: usize) -> Result<()> {
        self.read_exact(count).map(|_| ())
    }

    pub fn read_exact(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or_else(|| CueMolError::parse("XDR byte offset overflow"))?;
        let data = self
            .input
            .get(self.cursor..end)
            .ok_or_else(|| CueMolError::parse("XDR input is truncated"))?;
        self.cursor = end;
        Ok(data)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.read_exact(N)?
            .try_into()
            .map_err(|_| CueMolError::parse("XDR input is truncated"))
    }
}

fn append_scaled_coordinate(output: &mut Vec<f32>, coordinate: [i32; 3], precision: f32) {
    output.extend(coordinate.map(|value| value as f32 / precision));
}

fn coordinate_sizes(min: [i32; 3], max: [i32; 3]) -> Result<([u32; 3], [u32; 3], u32)> {
    let mut sizes = [0_u32; 3];
    for axis in 0..3 {
        let range = i64::from(max[axis]) - i64::from(min[axis]) + 1;
        sizes[axis] = u32::try_from(range)
            .map_err(|_| CueMolError::parse("XTC coordinate range is invalid"))?;
        if sizes[axis] == 0 {
            return Err(CueMolError::parse("XTC coordinate range is empty"));
        }
    }
    if sizes.iter().copied().fold(0, |value, size| value | size) > 0x00ff_ffff {
        let bits = sizes.map(integer_bit_count);
        Ok((sizes, bits, 0))
    } else {
        Ok((sizes, [0; 3], mixed_integer_bit_count(sizes)))
    }
}

fn integer_bit_count(size: u32) -> u32 {
    if size == u32::MAX {
        32
    } else {
        32 - size.leading_zeros()
    }
}

fn mixed_integer_bit_count(sizes: [u32; 3]) -> u32 {
    let mut bytes = [0_u8; 32];
    bytes[0] = 1;
    let mut byte_count = 1_usize;
    for size in sizes {
        let mut carry = 0_u64;
        for byte in bytes.iter_mut().take(byte_count) {
            let value = u64::from(*byte) * u64::from(size) + carry;
            *byte = value as u8;
            carry = value >> 8;
        }
        while carry != 0 {
            bytes[byte_count] = carry as u8;
            byte_count += 1;
            carry >>= 8;
        }
    }
    let highest = bytes[byte_count - 1];
    (byte_count as u32 - 1) * 8 + (8 - highest.leading_zeros())
}

fn decode_mixed_integers(
    bits: &mut BitReader<'_>,
    bit_count: u32,
    sizes: [u32; 3],
) -> Result<[i32; 3]> {
    if sizes.contains(&0) {
        return Err(CueMolError::parse("XTC zero mixed-integer coordinate size"));
    }

    let mut bytes = Vec::with_capacity(bit_count.div_ceil(8) as usize);
    let mut remaining = bit_count;
    while remaining >= 8 {
        bytes.push(bits.read_bits(8)? as u8);
        remaining -= 8;
    }
    if remaining > 0 {
        bytes.push(bits.read_bits(remaining)? as u8);
    }

    let mut output = [0_i32; 3];
    for axis in (1..=2).rev() {
        let divisor = u64::from(sizes[axis]);
        let mut remainder = 0_u64;
        for byte in bytes.iter_mut().rev() {
            let value = (remainder << 8) | u64::from(*byte);
            *byte = (value / divisor) as u8;
            remainder = value % divisor;
        }
        output[axis] = remainder as i32;
    }
    let mut first = 0_u64;
    for byte in bytes.iter().rev() {
        first = first
            .checked_mul(256)
            .and_then(|value| value.checked_add(u64::from(*byte)))
            .ok_or_else(|| CueMolError::parse("XTC mixed integer overflow"))?;
    }
    output[0] =
        i32::try_from(first).map_err(|_| CueMolError::parse("XTC mixed integer overflow"))?;
    Ok(output)
}

#[derive(Debug, Clone)]
struct BitReader<'a> {
    input: &'a [u8],
    bit_offset: usize,
}

impl<'a> BitReader<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            bit_offset: 0,
        }
    }

    fn read_bits(&mut self, count: u32) -> Result<u32> {
        if count > 32 {
            return Err(CueMolError::parse("XTC bit field is too wide"));
        }
        let end = self
            .bit_offset
            .checked_add(count as usize)
            .ok_or_else(|| CueMolError::parse("XTC bit offset overflow"))?;
        if end > self.input.len().saturating_mul(8) {
            return Err(CueMolError::parse("XTC compressed data is truncated"));
        }
        let mut value = 0_u32;
        for _ in 0..count {
            let byte = self.input[self.bit_offset / 8];
            let shift = 7 - self.bit_offset % 8;
            value = (value << 1) | u32::from((byte >> shift) & 1);
            self.bit_offset += 1;
        }
        Ok(value)
    }
}

fn norm(value: [f64; 3]) -> f64 {
    value
        .iter()
        .map(|component| component * component)
        .sum::<f64>()
        .sqrt()
}

fn vector_angle_degrees(lhs: [f64; 3], rhs: [f64; 3]) -> f64 {
    let lhs_norm = norm(lhs);
    let rhs_norm = norm(rhs);
    if lhs_norm == 0.0 || rhs_norm == 0.0 {
        return 90.0;
    }
    let dot = lhs
        .iter()
        .zip(rhs)
        .map(|(left, right)| left * right)
        .sum::<f64>();
    (dot / (lhs_norm * rhs_norm))
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_big_endian_scalars_and_gromacs_string() {
        let mut input = Vec::new();
        input.extend_from_slice(&(-7_i32).to_be_bytes());
        input.extend_from_slice(&1.25_f32.to_be_bytes());
        input.extend_from_slice(&2.5_f64.to_be_bytes());
        input.extend_from_slice(&4_u32.to_be_bytes());
        input.extend_from_slice(&3_u32.to_be_bytes());
        input.extend_from_slice(b"TRR");
        input.push(0);

        let mut xdr = XdrInStream::new(&input);
        assert_eq!(xdr.read_i32().unwrap(), -7);
        assert_eq!(xdr.read_f32().unwrap(), 1.25);
        assert_eq!(xdr.read_f64().unwrap(), 2.5);
        assert_eq!(xdr.read_gmx_string().unwrap(), "TRR");
        assert!(xdr.is_empty());
    }

    #[test]
    fn converts_triclinic_box_to_cell_parameters() {
        let mut input = Vec::new();
        for value in [1.0_f32, 0.0, 0.0, 0.5, 1.0, 0.0, 0.0, 0.0, 2.0] {
            input.extend_from_slice(&value.to_be_bytes());
        }
        let cell = XdrInStream::new(&input).read_gmx_box(false).unwrap();
        assert!((cell[0] - 10.0).abs() < 1.0e-6);
        assert!((cell[1] - 11.180_339_887).abs() < 1.0e-6);
        assert!((cell[2] - 20.0).abs() < 1.0e-6);
        assert!((cell[3] - 90.0).abs() < 1.0e-6);
        assert!((cell[4] - 90.0).abs() < 1.0e-6);
        assert!((cell[5] - 63.434_948_823).abs() < 1.0e-6);
    }

    #[test]
    fn distinguishes_clean_end_from_truncated_boundary() {
        assert_eq!(XdrInStream::new(&[]).read_i32_optional().unwrap(), None);
        assert!(XdrInStream::new(&[0, 0, 0]).read_i32_optional().is_err());
    }
}
