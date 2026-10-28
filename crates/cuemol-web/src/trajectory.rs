use cuemol_mdtools::{netcdf::NetcdfFileLayout, TrajectoryFrame};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::File;

fn error(value: impl ToString) -> JsValue {
    JsValue::from_str(&value.to_string())
}
async fn slice(file: &File, offset: u64, len: u64) -> Result<Vec<u8>, JsValue> {
    let end = offset
        .checked_add(len)
        .filter(|end| *end <= file.size() as u64)
        .ok_or_else(|| error("Truncated trajectory"))?;
    let blob = file.slice_with_f64_and_f64(offset as f64, end as f64)?;
    Ok(js_sys::Uint8Array::new(&JsFuture::from(blob.array_buffer()).await?).to_vec())
}
fn word(data: &[u8], offset: usize, little: bool) -> Result<u32, JsValue> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| error("Truncated frame header"))?;
    Ok(if little {
        u32::from_le_bytes(bytes.try_into().unwrap())
    } else {
        u32::from_be_bytes(bytes.try_into().unwrap())
    })
}

#[derive(Clone)]
enum Format {
    Dcd {
        prefix: Vec<u8>,
        start: u64,
        size: u64,
    },
    Mdcrd {
        has_box: bool,
    },
    Xtc,
    Trr,
    Netcdf(NetcdfFileLayout),
}
#[derive(Clone)]
pub struct BrowserTrajectory {
    file: File,
    format: Format,
    spans: Vec<(u64, u64)>,
    pub atoms: usize,
    pub frames: usize,
}
impl BrowserTrajectory {
    pub async fn open(file: File, expected: Option<usize>) -> Result<Self, JsValue> {
        let len = file.size() as u64;
        let header = slice(&file, 0, len.min(4096)).await?;
        let magic = word(&header, 0, false)?;
        let name = file.name().to_ascii_lowercase();
        if name.ends_with(".mdcrd") || name.ends_with(".crd") {
            let atoms = expected.ok_or_else(|| error("MDCRD requires a selected topology"))?;
            let mut index =
                cuemol_mdtools::mdcrd_index::MdcrdIndexer::new(atoms, 1, None).map_err(error)?;
            let mut offset = 0;
            while offset < len {
                let bytes = slice(&file, offset, (len - offset).min(262144)).await?;
                index.push(&bytes).map_err(error)?;
                offset += bytes.len() as u64;
            }
            let index = index.finish().map_err(error)?;
            let frames = index.spans.len();
            return Ok(Self {
                file,
                format: Format::Mdcrd {
                    has_box: index.has_box,
                },
                spans: index.spans,
                atoms,
                frames,
            });
        }
        if header.starts_with(b"CDF") {
            let mut bytes = header;
            let layout = loop {
                match NetcdfFileLayout::from_header(&bytes, len) {
                    Ok(layout) => break layout,
                    Err(e) if bytes.len() as u64 == len || bytes.len() >= 16 * 1024 * 1024 => {
                        return Err(error(e))
                    }
                    Err(_) => bytes = slice(&file, 0, len.min(bytes.len() as u64 * 2)).await?,
                }
            };
            let atoms = layout.atom_count();
            let frames = layout.frame_count();
            if frames == 0 {
                return Err(error("Trajectory has no frames"));
            }
            return Ok(Self {
                file,
                format: Format::Netcdf(layout),
                spans: Vec::new(),
                atoms,
                frames,
            });
        }
        if magic == 84 || word(&header, 0, true)? == 84 {
            let little = word(&header, 0, true)? == 84;
            if header.get(4..8) != Some(b"CORD")
                || word(&header, 40, little)? != 0
                || word(&header, 88, little)? != 84
            {
                return Err(error("Invalid DCD or unsupported fixed atoms"));
            }
            let frames = word(&header, 8, little)? as usize;
            let title = word(&header, 92, little)? as u64;
            let n = slice(&file, 96 + title, 16).await?;
            if word(&n, 0, little)? as u64 != title
                || word(&n, 4, little)? != 4
                || word(&n, 12, little)? != 4
            {
                return Err(error("Invalid DCD record markers"));
            }
            let atoms = word(&n, 8, little)? as usize;
            let start = 112 + title;
            let size = atoms as u64 * 12
                + 24
                + if word(&header, 48, little)? != 0 {
                    56
                } else {
                    0
                };
            if frames == 0 || atoms == 0 || frames as u64 > len.saturating_sub(start) / size {
                return Err(error("Truncated or empty DCD trajectory"));
            }
            let mut prefix = header[..92].to_vec();
            prefix[8..12].copy_from_slice(&if little {
                1_u32.to_le_bytes()
            } else {
                1_u32.to_be_bytes()
            });
            let four = if little {
                4_u32.to_le_bytes()
            } else {
                4_u32.to_be_bytes()
            };
            prefix.extend(four);
            prefix.extend([0; 4]);
            prefix.extend(four);
            prefix.extend(&n[4..]);
            return Ok(Self {
                file,
                format: Format::Dcd {
                    prefix,
                    start,
                    size,
                },
                spans: Vec::new(),
                atoms,
                frames,
            });
        }
        if !matches!(magic, 1993 | 1995 | 2023) {
            return Err(error("Expected DCD, XTC, TRR, or AMBER NetCDF"));
        }
        let xtc = magic != 1993;
        let mut spans = Vec::new();
        let mut atoms = 0;
        let mut offset = 0;
        let mut cache_start = 0;
        let mut cache = header;
        while offset < len {
            if offset < cache_start || offset + 256 > cache_start + cache.len() as u64 {
                cache_start = offset;
                cache = slice(&file, offset, (len - offset).min(65536)).await?;
            }
            let data = &cache[(offset - cache_start) as usize..];
            let m = word(data, 0, false)?;
            let (count, size, coords) = if xtc {
                if !matches!(m, 1995 | 2023) {
                    return Err(error("Invalid XTC magic"));
                }
                let n = word(data, 4, false)? as usize;
                if word(data, 52, false)? as usize != n {
                    return Err(error("XTC atom count mismatch"));
                }
                let size = if n <= 9 {
                    56 + n as u64 * 12
                } else {
                    let length = if m == 2023 {
                        ((word(data, 88, false)? as u64) << 32) | word(data, 92, false)? as u64
                    } else {
                        word(data, 88, false)? as u64
                    };
                    (if m == 2023 { 96 } else { 92 }) + ((length + 3) & !3)
                };
                (n, size, true)
            } else {
                if m != 1993 {
                    return Err(error("Invalid TRR magic"));
                }
                let declared = word(data, 4, false)?;
                let length = word(data, 8, false)?;
                if length > 128 || !(declared == length || declared == length + 1) {
                    return Err(error("Invalid TRR version"));
                }
                let p = 12 + ((length as usize + 3) & !3);
                let sizes = (0..10)
                    .map(|i| word(data, p + i * 4, false).map(u64::from))
                    .collect::<Result<Vec<_>, _>>()?;
                let n = word(data, p + 40, false)? as usize;
                let scalar = if sizes[2] > 0 {
                    sizes[2] / 9
                } else {
                    sizes[7..].iter().copied().find(|s| *s > 0).unwrap_or(0) / (n.max(1) as u64 * 3)
                };
                if !matches!(scalar, 4 | 8)
                    || sizes[7..]
                        .iter()
                        .any(|s| *s != 0 && *s != n as u64 * 3 * scalar)
                {
                    return Err(error("Invalid TRR block sizes"));
                }
                (
                    n,
                    p as u64 + 52 + 2 * scalar + sizes.iter().sum::<u64>(),
                    sizes[7] > 0,
                )
            };
            if count == 0 || (atoms != 0 && atoms != count) || size > len - offset {
                return Err(error("Invalid or truncated trajectory frame"));
            }
            atoms = count;
            if coords {
                spans.push((offset, size));
            }
            offset += size;
        }
        let frames = spans.len();
        if frames == 0 {
            return Err(error("Trajectory has no coordinate frames"));
        }
        Ok(Self {
            file,
            format: if xtc { Format::Xtc } else { Format::Trr },
            spans,
            atoms,
            frames,
        })
    }
    pub async fn frame(&self, index: usize) -> Result<TrajectoryFrame, JsValue> {
        if index >= self.frames {
            return Err(error("Frame out of range"));
        }
        let (offset, len) = match &self.format {
            Format::Dcd { start, size, .. } => (start + size * index as u64, *size),
            Format::Netcdf(layout) => {
                let (a, b) = layout.frame_span(index).map_err(error)?;
                (a, b as u64)
            }
            _ => self.spans[index],
        };
        let mut bytes = slice(&self.file, offset, len).await?;
        if let Format::Netcdf(layout) = &self.format {
            return layout.decode_frame(&bytes).map_err(error);
        }
        if let Format::Mdcrd { has_box } = self.format {
            return cuemol_mdtools::mdcrd_index::decode_mdcrd_frame(&bytes, self.atoms, has_box)
                .map_err(error);
        }
        let mut block = match &self.format {
            Format::Dcd { prefix, .. } => {
                let mut all = prefix.clone();
                all.append(&mut bytes);
                cuemol_mdtools::read_dcd_bytes(&all)
            }
            Format::Xtc => cuemol_mdtools::read_xtc_bytes(&bytes),
            Format::Trr => cuemol_mdtools::read_trr_bytes(&bytes),
            _ => unreachable!(),
        }
        .map_err(error)?;
        let cell = block.cell(0);
        Ok(TrajectoryFrame {
            coordinates: block
                .take_frame(0)
                .ok_or_else(|| error("Missing coordinates"))?,
            cell,
        })
    }
}
