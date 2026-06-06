use cuemol_core::{CueMolError, Result, Vector4};
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct GridMap {
    pub counts: [usize; 3],
    pub origin: Vector4,
    pub delta: [Vector4; 3],
    pub values: Vec<f64>,
}

impl GridMap {
    pub fn value_count(&self) -> usize {
        self.values.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ccp4Map {
    pub grid: GridMap,
    pub header: Ccp4Header,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ccp4Header {
    pub raw_counts: [usize; 3],
    pub counts: [usize; 3],
    pub start: [i32; 3],
    pub grid_intervals: [usize; 3],
    pub cell: [f64; 6],
    pub axis_order: [usize; 3],
    pub mode: i32,
    pub space_group: i32,
    pub symmetry_bytes: usize,
    pub stats: MapStats,
    pub byte_swapped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapStats {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub rms: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XplorMap {
    pub grid: GridMap,
    pub header: XplorHeader,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XplorHeader {
    pub counts: [usize; 3],
    pub start: [i32; 3],
    pub end: [i32; 3],
    pub grid_intervals: [usize; 3],
    pub cell: [f64; 6],
    pub title_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrixMap {
    pub grid: GridMap,
    pub header: BrixHeader,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrixHeader {
    pub counts: [usize; 3],
    pub start: [i32; 3],
    pub end: [i32; 3],
    pub grid_intervals: [usize; 3],
    pub cell: [f64; 6],
    pub prod: f64,
    pub plus: f64,
    pub sigma: f64,
    pub dsn6: bool,
    pub byte_swapped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MmcifMapStatus {
    GitLfsPointer { oid: String, size: usize },
    ParsedPlaceholder,
}

const CCP4_HEADER_LEN: usize = 1024;
const CCP4_MARKER_OFFSET: usize = 208;
const CCP4_MAX_DIM: i32 = 0x10000;

pub fn read_ccp4_map_file(path: impl AsRef<Path>) -> Result<GridMap> {
    Ok(read_ccp4_map_with_header_file(path)?.grid)
}

pub fn read_ccp4_map_bytes(bytes: &[u8]) -> Result<GridMap> {
    Ok(read_ccp4_map_with_header_bytes(bytes)?.grid)
}

pub fn read_ccp4_map_with_header_file(path: impl AsRef<Path>) -> Result<Ccp4Map> {
    let bytes = crate::fileio::read_bytes_maybe_compressed(path)?;
    read_ccp4_map_with_header_bytes(&bytes)
}

pub fn read_ccp4_map_with_header_bytes(bytes: &[u8]) -> Result<Ccp4Map> {
    if bytes.len() < CCP4_HEADER_LEN {
        return Err(CueMolError::parse("CCP4 header is too short"));
    }
    if bytes.get(CCP4_MARKER_OFFSET..CCP4_MARKER_OFFSET + 4) != Some(b"MAP ".as_slice()) {
        return Err(CueMolError::parse("missing CCP4 MAP marker"));
    }

    let byte_swapped = detect_ccp4_byte_order(bytes)?;
    let raw_counts = [
        read_i32_word(bytes, 0, byte_swapped),
        read_i32_word(bytes, 1, byte_swapped),
        read_i32_word(bytes, 2, byte_swapped),
    ];
    let raw_counts_usize = [
        valid_dim(raw_counts[0], "invalid CCP4 column count")?,
        valid_dim(raw_counts[1], "invalid CCP4 row count")?,
        valid_dim(raw_counts[2], "invalid CCP4 section count")?,
    ];
    let mode = read_i32_word(bytes, 3, byte_swapped);
    let raw_start = [
        read_i32_word(bytes, 4, byte_swapped),
        read_i32_word(bytes, 5, byte_swapped),
        read_i32_word(bytes, 6, byte_swapped),
    ];
    let grid_intervals = [
        positive_or(raw_counts_usize[0], read_i32_word(bytes, 7, byte_swapped)),
        positive_or(raw_counts_usize[1], read_i32_word(bytes, 8, byte_swapped)),
        positive_or(raw_counts_usize[2], read_i32_word(bytes, 9, byte_swapped)),
    ];
    let cell = [
        read_f32_word(bytes, 10, byte_swapped) as f64,
        read_f32_word(bytes, 11, byte_swapped) as f64,
        read_f32_word(bytes, 12, byte_swapped) as f64,
        read_f32_word(bytes, 13, byte_swapped) as f64,
        read_f32_word(bytes, 14, byte_swapped) as f64,
        read_f32_word(bytes, 15, byte_swapped) as f64,
    ];
    let axis_order = parse_ccp4_axis_order([
        read_i32_word(bytes, 16, byte_swapped),
        read_i32_word(bytes, 17, byte_swapped),
        read_i32_word(bytes, 18, byte_swapped),
    ])?;
    let stats = MapStats {
        min: read_f32_word(bytes, 19, byte_swapped) as f64,
        max: read_f32_word(bytes, 20, byte_swapped) as f64,
        mean: read_f32_word(bytes, 21, byte_swapped) as f64,
        rms: read_f32_word(bytes, 54, byte_swapped) as f64,
    };
    let space_group = read_i32_word(bytes, 22, byte_swapped);
    let symmetry_bytes = read_i32_word(bytes, 23, byte_swapped);
    if symmetry_bytes < 0 {
        return Err(CueMolError::parse("invalid CCP4 symmetry byte count"));
    }
    let symmetry_bytes = symmetry_bytes as usize;

    let raw_value_count = raw_counts_usize
        .into_iter()
        .try_fold(1_usize, |acc, value| acc.checked_mul(value))
        .ok_or_else(|| CueMolError::parse("CCP4 map size overflow"))?;
    let data_start = CCP4_HEADER_LEN
        .checked_add(symmetry_bytes)
        .ok_or_else(|| CueMolError::parse("CCP4 data offset overflow"))?;
    let raw_values = read_ccp4_values(bytes, data_start, raw_value_count, mode, byte_swapped)?;

    let counts = rotate_usize(raw_counts_usize, axis_order);
    let start = rotate_i32(raw_start, axis_order);
    let values = rotate_values(&raw_values, raw_counts_usize, axis_order, counts);
    let delta = cell_delta_vectors(cell, grid_intervals)?;
    let origin = delta[0]
        .scale(start[0] as f64)
        .add_vec(delta[1].scale(start[1] as f64))
        .add_vec(delta[2].scale(start[2] as f64));

    Ok(Ccp4Map {
        grid: GridMap {
            counts,
            origin,
            delta,
            values,
        },
        header: Ccp4Header {
            raw_counts: raw_counts_usize,
            counts,
            start,
            grid_intervals,
            cell,
            axis_order,
            mode,
            space_group,
            symmetry_bytes,
            stats,
            byte_swapped,
        },
    })
}

pub fn read_opendx_file(path: impl AsRef<Path>) -> Result<GridMap> {
    let input = crate::fileio::read_text_maybe_compressed(path)?;
    read_opendx_str(&input)
}

pub fn read_xplor_map_file(path: impl AsRef<Path>) -> Result<GridMap> {
    Ok(read_xplor_map_with_header_file(path)?.grid)
}

pub fn read_xplor_map_str(input: &str) -> Result<GridMap> {
    Ok(read_xplor_map_with_header_str(input)?.grid)
}

pub fn read_xplor_map_with_header_file(path: impl AsRef<Path>) -> Result<XplorMap> {
    let input = crate::fileio::read_text_maybe_compressed(path)?;
    read_xplor_map_with_header_str(&input)
}

pub fn read_xplor_map_with_header_str(input: &str) -> Result<XplorMap> {
    let lines = input.lines().collect::<Vec<_>>();
    let mut index = 0;
    let title_count = read_xplor_title_count(&lines, &mut index)?;
    if title_count > 1000 {
        return Err(CueMolError::parse("invalid Xplor title count"));
    }
    skip_xplor_lines(&lines, &mut index, title_count)?;

    let section = find_xplor_line(&lines, &mut index, 10, parse_xplor_section_info)?
        .ok_or_else(|| CueMolError::parse("missing Xplor section info"))?;
    let cell = find_xplor_line(&lines, &mut index, 10, parse_xplor_cell_info)?
        .ok_or_else(|| CueMolError::parse("missing Xplor cell info"))?;
    find_xplor_line(&lines, &mut index, 10, parse_xplor_axis_info)?
        .ok_or_else(|| CueMolError::parse("missing Xplor ZYX axis line"))?;

    let counts = [
        valid_xplor_count(section.end[0] - section.start[0] + 1)?,
        valid_xplor_count(section.end[1] - section.start[1] + 1)?,
        valid_xplor_count(section.end[2] - section.start[2] + 1)?,
    ];
    let grid_intervals = [
        valid_xplor_count(section.grid_intervals[0])?,
        valid_xplor_count(section.grid_intervals[1])?,
        valid_xplor_count(section.grid_intervals[2])?,
    ];
    let value_count = counts
        .into_iter()
        .try_fold(1_usize, |acc, value| acc.checked_mul(value))
        .ok_or_else(|| CueMolError::parse("Xplor map size overflow"))?;
    let mut values = Vec::with_capacity(value_count);
    let section_size = counts[0] * counts[1];
    for section_index in 0..counts[2] {
        let section_number = read_next_xplor_section_number(&lines, &mut index)?;
        let expected_zero_based = section_index as i32;
        let expected_start_based = section.start[2] + section_index as i32;
        if section_number != expected_zero_based && section_number != expected_start_based {
            return Err(CueMolError::parse("unexpected Xplor section number"));
        }
        while values.len() < (section_index + 1) * section_size {
            let line = lines
                .get(index)
                .ok_or_else(|| CueMolError::parse("missing Xplor density values"))?;
            index += 1;
            values.extend(parse_xplor_density_line(line)?);
        }
    }
    if values.len() != value_count {
        return Err(CueMolError::parse("Xplor data count mismatch"));
    }

    let delta = cell_delta_vectors(cell, grid_intervals)?;
    let origin = delta[0]
        .scale(section.start[0] as f64)
        .add_vec(delta[1].scale(section.start[1] as f64))
        .add_vec(delta[2].scale(section.start[2] as f64));
    Ok(XplorMap {
        grid: GridMap {
            counts,
            origin,
            delta,
            values,
        },
        header: XplorHeader {
            counts,
            start: section.start,
            end: section.end,
            grid_intervals,
            cell,
            title_count,
        },
    })
}

pub fn read_brix_map_file(path: impl AsRef<Path>) -> Result<GridMap> {
    Ok(read_brix_map_with_header_file(path)?.grid)
}

pub fn read_brix_map_bytes(bytes: &[u8]) -> Result<GridMap> {
    Ok(read_brix_map_with_header_bytes(bytes)?.grid)
}

pub fn read_brix_map_with_header_file(path: impl AsRef<Path>) -> Result<BrixMap> {
    let bytes = crate::fileio::read_bytes_maybe_compressed(path)?;
    read_brix_map_with_header_bytes(&bytes)
}

pub fn read_brix_map_with_header_bytes(bytes: &[u8]) -> Result<BrixMap> {
    if bytes.len() < 512 {
        return Err(CueMolError::parse("BRIX header is too short"));
    }
    let header = parse_brix_header(&bytes[..512])?;
    let xbri = header.counts[0].div_ceil(8);
    let ybri = header.counts[1].div_ceil(8);
    let zbri = header.counts[2].div_ceil(8);
    let block_count = xbri
        .checked_mul(ybri)
        .and_then(|value| value.checked_mul(zbri))
        .ok_or_else(|| CueMolError::parse("BRIX block count overflow"))?;
    let data_len = block_count
        .checked_mul(512)
        .ok_or_else(|| CueMolError::parse("BRIX data length overflow"))?;
    if bytes.len() < 512 + data_len {
        return Err(CueMolError::parse("BRIX data is shorter than header count"));
    }

    let mut values = vec![0.0; header.counts[0] * header.counts[1] * header.counts[2]];
    let mut offset = 512;
    for ibz in 0..zbri {
        for iby in 0..ybri {
            for ibx in 0..xbri {
                let block = &bytes[offset..offset + 512];
                offset += 512;
                let bx = ibx * 8;
                let by = iby * 8;
                let bz = ibz * 8;
                for iz in 0..8 {
                    for iy in 0..8 {
                        for ix in 0..8 {
                            let x = bx + ix;
                            let y = by + iy;
                            let z = bz + iz;
                            if x >= header.counts[0]
                                || y >= header.counts[1]
                                || z >= header.counts[2]
                            {
                                continue;
                            }
                            let raw = block[ix + (iy + iz * 8) * 8] as f64;
                            let out_index = x + (y + z * header.counts[1]) * header.counts[0];
                            values[out_index] = (raw - header.plus) / header.prod;
                        }
                    }
                }
            }
        }
    }

    let delta = cell_delta_vectors(header.cell, header.grid_intervals)?;
    let origin = delta[0]
        .scale(header.start[0] as f64)
        .add_vec(delta[1].scale(header.start[1] as f64))
        .add_vec(delta[2].scale(header.start[2] as f64));
    Ok(BrixMap {
        grid: GridMap {
            counts: header.counts,
            origin,
            delta,
            values,
        },
        header,
    })
}

pub fn read_opendx_str(input: &str) -> Result<GridMap> {
    let mut counts = None;
    let mut origin = None;
    let mut deltas = Vec::new();
    let mut expected_items = None;
    let mut values = Vec::new();
    let mut reading_data = false;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with("object 1 class gridpositions counts") {
            let nums = parse_usize_tail(trimmed, 3)?;
            counts = Some([nums[0], nums[1], nums[2]]);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("origin") {
            let nums = parse_f64_values(rest, 3)?;
            origin = Some(Vector4::new3(nums[0], nums[1], nums[2]));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("delta") {
            let nums = parse_f64_values(rest, 3)?;
            deltas.push(Vector4::new3(nums[0], nums[1], nums[2]));
            continue;
        }
        if trimmed.starts_with("object 3 class array") {
            let tokens = trimmed.split_whitespace().collect::<Vec<_>>();
            if let Some(index) = tokens.iter().position(|token| *token == "items") {
                expected_items = tokens.get(index + 1).and_then(|value| value.parse().ok());
            }
            reading_data = trimmed.contains("data follows");
            continue;
        }
        if reading_data {
            if trimmed.starts_with("attribute ")
                || trimmed.starts_with("object ")
                || trimmed.starts_with("component ")
            {
                reading_data = false;
                continue;
            }
            for token in trimmed.split_whitespace() {
                values.push(
                    token
                        .parse::<f64>()
                        .map_err(|_| CueMolError::parse("invalid OpenDX data value"))?,
                );
            }
        }
    }

    let counts = counts.ok_or_else(|| CueMolError::parse("missing OpenDX grid counts"))?;
    let origin = origin.ok_or_else(|| CueMolError::parse("missing OpenDX origin"))?;
    if deltas.len() != 3 {
        return Err(CueMolError::parse("missing OpenDX delta vectors"));
    }
    let expected_items = expected_items.unwrap_or(counts[0] * counts[1] * counts[2]);
    if values.len() != expected_items {
        return Err(CueMolError::parse("OpenDX data count mismatch"));
    }
    Ok(GridMap {
        counts,
        origin,
        delta: [deltas[0], deltas[1], deltas[2]],
        values,
    })
}

pub fn read_mmcif_map_file(path: impl AsRef<Path>) -> Result<MmcifMapStatus> {
    let input = crate::fileio::read_text_maybe_compressed(path)?;
    if input.starts_with("version https://git-lfs.github.com/spec/v1") {
        let oid = input
            .lines()
            .find_map(|line| line.strip_prefix("oid sha256:").map(str::to_string))
            .unwrap_or_default();
        let size = input
            .lines()
            .find_map(|line| {
                line.strip_prefix("size ")
                    .and_then(|value| value.parse().ok())
            })
            .unwrap_or(0);
        return Ok(MmcifMapStatus::GitLfsPointer { oid, size });
    }
    Ok(MmcifMapStatus::ParsedPlaceholder)
}

fn parse_brix_header(bytes: &[u8]) -> Result<BrixHeader> {
    let text = String::from_utf8_lossy(bytes);
    let tokens = text
        .split(|ch: char| ch == ',' || ch.is_ascii_whitespace() || ch == '\0')
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.first().copied() != Some(":-)") {
        return parse_dsn6_header(bytes);
    }

    let mut index = 1;
    expect_brix_label(&tokens, &mut index, "origin")?;
    let start = [
        parse_brix_i32(&tokens, &mut index, "invalid BRIX origin x")?,
        parse_brix_i32(&tokens, &mut index, "invalid BRIX origin y")?,
        parse_brix_i32(&tokens, &mut index, "invalid BRIX origin z")?,
    ];
    expect_brix_label(&tokens, &mut index, "extent")?;
    let counts_i32 = [
        parse_brix_i32(&tokens, &mut index, "invalid BRIX extent x")?,
        parse_brix_i32(&tokens, &mut index, "invalid BRIX extent y")?,
        parse_brix_i32(&tokens, &mut index, "invalid BRIX extent z")?,
    ];
    expect_brix_label(&tokens, &mut index, "grid")?;
    let grid_i32 = [
        parse_brix_i32(&tokens, &mut index, "invalid BRIX grid x")?,
        parse_brix_i32(&tokens, &mut index, "invalid BRIX grid y")?,
        parse_brix_i32(&tokens, &mut index, "invalid BRIX grid z")?,
    ];
    expect_brix_label(&tokens, &mut index, "cell")?;
    let cell = [
        parse_brix_f64(&tokens, &mut index, "invalid BRIX cell a")?,
        parse_brix_f64(&tokens, &mut index, "invalid BRIX cell b")?,
        parse_brix_f64(&tokens, &mut index, "invalid BRIX cell c")?,
        parse_brix_f64(&tokens, &mut index, "invalid BRIX cell alpha")?,
        parse_brix_f64(&tokens, &mut index, "invalid BRIX cell beta")?,
        parse_brix_f64(&tokens, &mut index, "invalid BRIX cell gamma")?,
    ];
    expect_brix_label(&tokens, &mut index, "prod")?;
    let prod = parse_brix_f64(&tokens, &mut index, "invalid BRIX prod")?;
    expect_brix_label(&tokens, &mut index, "plus")?;
    let plus = parse_brix_f64(&tokens, &mut index, "invalid BRIX plus")?;
    expect_brix_label(&tokens, &mut index, "sigma")?;
    let sigma = parse_brix_f64(&tokens, &mut index, "invalid BRIX sigma")?;
    if prod == 0.0 {
        return Err(CueMolError::parse("invalid BRIX prod"));
    }

    let counts = [
        valid_xplor_count(counts_i32[0])?,
        valid_xplor_count(counts_i32[1])?,
        valid_xplor_count(counts_i32[2])?,
    ];
    Ok(BrixHeader {
        counts,
        start,
        end: [
            start[0] + counts_i32[0],
            start[1] + counts_i32[1],
            start[2] + counts_i32[2],
        ],
        grid_intervals: [
            valid_xplor_count(grid_i32[0])?,
            valid_xplor_count(grid_i32[1])?,
            valid_xplor_count(grid_i32[2])?,
        ],
        cell,
        prod,
        plus,
        sigma,
        dsn6: false,
        byte_swapped: false,
    })
}

fn parse_dsn6_header(bytes: &[u8]) -> Result<BrixHeader> {
    let mut header = read_dsn6_header(bytes, false);
    let mut byte_swapped = false;
    if header[18] != 100 {
        header = read_dsn6_header(bytes, true);
        byte_swapped = true;
    }
    if header[18] != 100 {
        return Err(CueMolError::parse("invalid DSN6 header"));
    }
    let scale = header[17] as f64;
    if scale == 0.0 || header[15] == 0 {
        return Err(CueMolError::parse("invalid DSN6 scale"));
    }

    let counts = [
        valid_xplor_count(header[3] as i32)?,
        valid_xplor_count(header[4] as i32)?,
        valid_xplor_count(header[5] as i32)?,
    ];
    let prod = header[15] as f64 / header[18] as f64;
    let plus = header[16] as f64;
    Ok(BrixHeader {
        counts,
        start: [header[0] as i32, header[1] as i32, header[2] as i32],
        end: [
            header[0] as i32 + header[3] as i32,
            header[1] as i32 + header[4] as i32,
            header[2] as i32 + header[5] as i32,
        ],
        grid_intervals: [
            valid_xplor_count(header[6] as i32)?,
            valid_xplor_count(header[7] as i32)?,
            valid_xplor_count(header[8] as i32)?,
        ],
        cell: [
            header[9] as f64 / scale,
            header[10] as f64 / scale,
            header[11] as f64 / scale,
            header[12] as f64 / scale,
            header[13] as f64 / scale,
            header[14] as f64 / scale,
        ],
        prod,
        plus,
        sigma: 0.0,
        dsn6: true,
        byte_swapped,
    })
}

fn read_dsn6_header(bytes: &[u8], byte_swapped: bool) -> [i16; 19] {
    let mut header = [0_i16; 19];
    for (index, value) in header.iter_mut().enumerate() {
        let offset = index * 2;
        let field = [bytes[offset], bytes[offset + 1]];
        *value = if byte_swapped {
            i16::from_be_bytes(field)
        } else {
            i16::from_le_bytes(field)
        };
    }
    header
}

fn expect_brix_label(tokens: &[&str], index: &mut usize, expected: &str) -> Result<()> {
    let Some(token) = tokens.get(*index) else {
        return Err(CueMolError::parse(format!("missing BRIX {expected} label")));
    };
    *index += 1;
    if token.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(CueMolError::parse(format!("missing BRIX {expected} label")))
    }
}

fn parse_brix_i32(tokens: &[&str], index: &mut usize, message: &'static str) -> Result<i32> {
    let Some(token) = tokens.get(*index) else {
        return Err(CueMolError::parse(message));
    };
    *index += 1;
    token
        .parse::<i32>()
        .map_err(|_| CueMolError::parse(message))
}

fn parse_brix_f64(tokens: &[&str], index: &mut usize, message: &'static str) -> Result<f64> {
    let Some(token) = tokens.get(*index) else {
        return Err(CueMolError::parse(message));
    };
    *index += 1;
    token
        .parse::<f64>()
        .map_err(|_| CueMolError::parse(message))
}

#[derive(Debug, Clone, Copy)]
struct XplorSectionInfo {
    grid_intervals: [i32; 3],
    start: [i32; 3],
    end: [i32; 3],
}

fn read_xplor_title_count(lines: &[&str], index: &mut usize) -> Result<usize> {
    while let Some(line) = lines.get(*index) {
        *index += 1;
        let head = fixed_prefix(line, 8).trim();
        if head.is_empty() {
            continue;
        }
        if let Ok(value) = head.parse::<isize>() {
            if value < 0 {
                return Err(CueMolError::parse("invalid Xplor title count"));
            }
            return Ok(value as usize);
        }
    }
    Err(CueMolError::parse("missing Xplor title count"))
}

fn skip_xplor_lines(lines: &[&str], index: &mut usize, count: usize) -> Result<()> {
    if lines.len().saturating_sub(*index) < count {
        return Err(CueMolError::parse("missing Xplor title lines"));
    }
    *index += count;
    Ok(())
}

fn find_xplor_line<T>(
    lines: &[&str],
    index: &mut usize,
    max_lines: usize,
    parser: fn(&str) -> Result<Option<T>>,
) -> Result<Option<T>> {
    for _ in 0..max_lines {
        let Some(line) = lines.get(*index) else {
            return Ok(None);
        };
        *index += 1;
        if let Some(value) = parser(line.trim())? {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

fn parse_xplor_section_info(line: &str) -> Result<Option<XplorSectionInfo>> {
    let Some(values) = parse_i32_fields(line, 8, 9)? else {
        return Ok(None);
    };
    Ok(Some(XplorSectionInfo {
        grid_intervals: [values[0], values[3], values[6]],
        start: [values[1], values[4], values[7]],
        end: [values[2], values[5], values[8]],
    }))
}

fn parse_xplor_cell_info(line: &str) -> Result<Option<[f64; 6]>> {
    parse_f64_fields(line, 12, 6)
}

fn parse_xplor_axis_info(line: &str) -> Result<Option<()>> {
    Ok(line.trim_start().starts_with("ZYX").then_some(()))
}

fn read_next_xplor_section_number(lines: &[&str], index: &mut usize) -> Result<i32> {
    while let Some(line) = lines.get(*index) {
        *index += 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        return trimmed
            .parse::<i32>()
            .map_err(|_| CueMolError::parse("invalid Xplor section number"));
    }
    Err(CueMolError::parse("missing Xplor section number"))
}

fn parse_xplor_density_line(line: &str) -> Result<Vec<f64>> {
    if line.trim().is_empty() {
        return Ok(Vec::new());
    }
    if let Some(values) = parse_fixed_f64_values(line, 12)? {
        return Ok(values);
    }
    line.split_whitespace()
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| CueMolError::parse("invalid Xplor density value"))
        })
        .collect()
}

fn parse_i32_fields(line: &str, width: usize, count: usize) -> Result<Option<[i32; 9]>> {
    let values = parse_fixed_fields(line, width, count)
        .or_else(|| parse_whitespace_fields(line, count))
        .map(|fields| {
            fields
                .into_iter()
                .map(|value| {
                    value
                        .parse::<i32>()
                        .map_err(|_| CueMolError::parse("invalid Xplor integer field"))
                })
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;
    let Some(values) = values else {
        return Ok(None);
    };
    values
        .try_into()
        .map(Some)
        .map_err(|_| CueMolError::parse("wrong Xplor integer field count"))
}

fn parse_f64_fields(line: &str, width: usize, count: usize) -> Result<Option<[f64; 6]>> {
    let values = parse_fixed_fields(line, width, count)
        .or_else(|| parse_whitespace_fields(line, count))
        .map(|fields| {
            fields
                .into_iter()
                .map(|value| {
                    value
                        .parse::<f64>()
                        .map_err(|_| CueMolError::parse("invalid Xplor float field"))
                })
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;
    let Some(values) = values else {
        return Ok(None);
    };
    values
        .try_into()
        .map(Some)
        .map_err(|_| CueMolError::parse("wrong Xplor float field count"))
}

fn parse_fixed_f64_values(line: &str, width: usize) -> Result<Option<Vec<f64>>> {
    let chunks = line.len() / width;
    if chunks == 0 || chunks > 6 {
        return Ok(None);
    }
    let Some(fields) = parse_fixed_fields(line, width, chunks) else {
        return Ok(None);
    };
    let mut values = Vec::with_capacity(fields.len());
    for field in fields {
        if field.is_empty() {
            continue;
        }
        let Ok(value) = field.parse::<f64>() else {
            return Ok(None);
        };
        values.push(value);
    }
    Ok((!values.is_empty()).then_some(values))
}

fn parse_fixed_fields(line: &str, width: usize, count: usize) -> Option<Vec<String>> {
    let bytes = line.as_bytes();
    if bytes.len() < width * count {
        return None;
    }
    let mut fields = Vec::with_capacity(count);
    for index in 0..count {
        let start = index * width;
        let end = start + width;
        let field = std::str::from_utf8(&bytes[start..end]).ok()?.trim();
        fields.push(field.to_string());
    }
    Some(fields)
}

fn parse_whitespace_fields(line: &str, count: usize) -> Option<Vec<String>> {
    let fields = line
        .split_whitespace()
        .take(count)
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    (fields.len() == count).then_some(fields)
}

fn fixed_prefix(line: &str, width: usize) -> &str {
    if line.len() <= width {
        return line;
    }
    std::str::from_utf8(&line.as_bytes()[..width]).unwrap_or(line)
}

fn valid_xplor_count(value: i32) -> Result<usize> {
    if value <= 0 {
        Err(CueMolError::parse("invalid Xplor map count"))
    } else {
        Ok(value as usize)
    }
}

fn detect_ccp4_byte_order(bytes: &[u8]) -> Result<bool> {
    let le_counts = [
        read_i32_word(bytes, 0, false),
        read_i32_word(bytes, 1, false),
        read_i32_word(bytes, 2, false),
    ];
    let le_mode = read_i32_word(bytes, 3, false);
    if plausible_ccp4_header(le_counts, le_mode) {
        return Ok(false);
    }

    let be_counts = [
        read_i32_word(bytes, 0, true),
        read_i32_word(bytes, 1, true),
        read_i32_word(bytes, 2, true),
    ];
    let be_mode = read_i32_word(bytes, 3, true);
    if plausible_ccp4_header(be_counts, be_mode) {
        return Ok(true);
    }

    Err(CueMolError::parse("invalid CCP4 dimensions or mode"))
}

fn plausible_ccp4_header(counts: [i32; 3], mode: i32) -> bool {
    counts
        .into_iter()
        .all(|value| (1..=CCP4_MAX_DIM).contains(&value))
        && matches!(mode, 0 | 1 | 2 | 6)
}

fn valid_dim(value: i32, message: &'static str) -> Result<usize> {
    if (1..=CCP4_MAX_DIM).contains(&value) {
        Ok(value as usize)
    } else {
        Err(CueMolError::parse(message))
    }
}

fn positive_or(fallback: usize, value: i32) -> usize {
    if value > 0 {
        value as usize
    } else {
        fallback
    }
}

fn parse_ccp4_axis_order(values: [i32; 3]) -> Result<[usize; 3]> {
    let mut axes = [0_usize; 3];
    let mut seen = [false; 3];
    for (index, value) in values.into_iter().enumerate() {
        if !(1..=3).contains(&value) {
            return Err(CueMolError::parse("invalid CCP4 axis order"));
        }
        let axis = (value - 1) as usize;
        if seen[axis] {
            return Err(CueMolError::parse("duplicate CCP4 axis order"));
        }
        seen[axis] = true;
        axes[index] = axis;
    }
    Ok(axes)
}

fn read_ccp4_values(
    bytes: &[u8],
    data_start: usize,
    count: usize,
    mode: i32,
    byte_swapped: bool,
) -> Result<Vec<f64>> {
    let item_size = match mode {
        0 => 1,
        1 | 6 => 2,
        2 => 4,
        _ => return Err(CueMolError::parse(format!("unsupported CCP4 mode {mode}"))),
    };
    let byte_len = count
        .checked_mul(item_size)
        .ok_or_else(|| CueMolError::parse("CCP4 data size overflow"))?;
    let data_end = data_start
        .checked_add(byte_len)
        .ok_or_else(|| CueMolError::parse("CCP4 data end overflow"))?;
    if bytes.len() < data_end {
        return Err(CueMolError::parse("CCP4 data is shorter than header count"));
    }

    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        let offset = data_start + index * item_size;
        let value = match mode {
            0 => bytes[offset] as f64,
            1 => read_i16(bytes, offset, byte_swapped) as f64,
            2 => {
                let value = read_f32(bytes, offset, byte_swapped);
                if !value.is_finite() {
                    return Err(CueMolError::parse("invalid CCP4 float data"));
                }
                value as f64
            }
            6 => read_u16(bytes, offset, byte_swapped) as f64,
            _ => unreachable!(),
        };
        values.push(value);
    }
    Ok(values)
}

fn rotate_values(
    raw_values: &[f64],
    raw_counts: [usize; 3],
    axis_order: [usize; 3],
    counts: [usize; 3],
) -> Vec<f64> {
    let mut values = vec![0.0; raw_values.len()];
    for k in 0..raw_counts[2] {
        for j in 0..raw_counts[1] {
            for i in 0..raw_counts[0] {
                let raw_index = i + (j + k * raw_counts[1]) * raw_counts[0];
                let rotated = rotate_usize([i, j, k], axis_order);
                let out_index = rotated[0] + (rotated[1] + rotated[2] * counts[1]) * counts[0];
                values[out_index] = raw_values[raw_index];
            }
        }
    }
    values
}

fn rotate_usize(values: [usize; 3], axis_order: [usize; 3]) -> [usize; 3] {
    let mut rotated = [0_usize; 3];
    rotated[axis_order[0]] = values[0];
    rotated[axis_order[1]] = values[1];
    rotated[axis_order[2]] = values[2];
    rotated
}

fn rotate_i32(values: [i32; 3], axis_order: [usize; 3]) -> [i32; 3] {
    let mut rotated = [0_i32; 3];
    rotated[axis_order[0]] = values[0];
    rotated[axis_order[1]] = values[1];
    rotated[axis_order[2]] = values[2];
    rotated
}

fn cell_delta_vectors(cell: [f64; 6], grid_intervals: [usize; 3]) -> Result<[Vector4; 3]> {
    if grid_intervals.contains(&0) {
        return Err(CueMolError::parse("invalid CCP4 grid interval"));
    }

    let basis = cell_basis(cell)?;
    Ok([
        basis[0].divide(grid_intervals[0] as f64)?,
        basis[1].divide(grid_intervals[1] as f64)?,
        basis[2].divide(grid_intervals[2] as f64)?,
    ])
}

fn cell_basis(cell: [f64; 6]) -> Result<[Vector4; 3]> {
    let [a, b, c, alpha, beta, gamma] = cell;
    if a <= 0.0 || b <= 0.0 || c <= 0.0 {
        return Err(CueMolError::parse("invalid CCP4 unit cell length"));
    }
    let alpha = alpha.to_radians();
    let beta = beta.to_radians();
    let gamma = gamma.to_radians();
    let sin_gamma = gamma.sin();
    if sin_gamma.abs() < 1.0e-12 {
        return Err(CueMolError::parse("invalid CCP4 unit cell angle"));
    }

    let ax = Vector4::new3(a, 0.0, 0.0);
    let bx = b * gamma.cos();
    let by = b * sin_gamma;
    let cx = c * beta.cos();
    let cy = c * (alpha.cos() - beta.cos() * gamma.cos()) / sin_gamma;
    let cz_sq = c * c - cx * cx - cy * cy;
    if cz_sq < -1.0e-8 {
        return Err(CueMolError::parse("invalid CCP4 unit cell geometry"));
    }
    Ok([
        ax,
        Vector4::new3(bx, by, 0.0),
        Vector4::new3(cx, cy, cz_sq.max(0.0).sqrt()),
    ])
}

fn read_i32_word(bytes: &[u8], word: usize, byte_swapped: bool) -> i32 {
    read_i32(bytes, word * 4, byte_swapped)
}

fn read_f32_word(bytes: &[u8], word: usize, byte_swapped: bool) -> f32 {
    read_f32(bytes, word * 4, byte_swapped)
}

fn read_i32(bytes: &[u8], offset: usize, byte_swapped: bool) -> i32 {
    let field = bytes[offset..offset + 4].try_into().unwrap();
    if byte_swapped {
        i32::from_be_bytes(field)
    } else {
        i32::from_le_bytes(field)
    }
}

fn read_i16(bytes: &[u8], offset: usize, byte_swapped: bool) -> i16 {
    let field = bytes[offset..offset + 2].try_into().unwrap();
    if byte_swapped {
        i16::from_be_bytes(field)
    } else {
        i16::from_le_bytes(field)
    }
}

fn read_u16(bytes: &[u8], offset: usize, byte_swapped: bool) -> u16 {
    let field = bytes[offset..offset + 2].try_into().unwrap();
    if byte_swapped {
        u16::from_be_bytes(field)
    } else {
        u16::from_le_bytes(field)
    }
}

fn read_f32(bytes: &[u8], offset: usize, byte_swapped: bool) -> f32 {
    let field = bytes[offset..offset + 4].try_into().unwrap();
    if byte_swapped {
        f32::from_be_bytes(field)
    } else {
        f32::from_le_bytes(field)
    }
}

fn parse_usize_tail(input: &str, count: usize) -> Result<Vec<usize>> {
    let values = input
        .split_whitespace()
        .rev()
        .take(count)
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| CueMolError::parse("invalid OpenDX integer"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(values.into_iter().rev().collect())
}

fn parse_f64_values(input: &str, count: usize) -> Result<Vec<f64>> {
    let values = input
        .split_whitespace()
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| CueMolError::parse("invalid OpenDX float"))
        })
        .collect::<Result<Vec<_>>>()?;
    if values.len() != count {
        return Err(CueMolError::parse("wrong OpenDX vector length"));
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_small_opendx_grid() {
        let dx = "\
object 1 class gridpositions counts  1 1 2
origin 0.0 1.0 2.0
delta  1 0 0
delta  0 1 0
delta  0 0 1
object 2 class gridconnections counts  1 1 2
object 3 class array type \"double\" rank 0 items 2 data follows
1.0 2.0
attribute \"dep\" string \"positions\"
";
        let map = read_opendx_str(dx).unwrap();
        assert_eq!(map.counts, [1, 1, 2]);
        assert_eq!(map.value_count(), 2);
        assert_eq!(map.values, vec![1.0, 2.0]);
    }
}
