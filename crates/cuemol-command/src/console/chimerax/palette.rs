use super::colors;
#[derive(Clone, Debug)]
pub(super) struct Palette(pub Vec<(f64, [f64; 3])>);
fn hex(rgb: [f64; 3]) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        rgb[0].clamp(0., 255.).round() as u8,
        rgb[1].clamp(0., 255.).round() as u8,
        rgb[2].clamp(0., 255.).round() as u8
    )
}
fn rgb(hex: &str) -> [f64; 3] {
    std::array::from_fn(|i| u8::from_str_radix(&hex[1 + 2 * i..3 + 2 * i], 16).unwrap() as f64)
}
const CHAINS: [[f64; 3]; 36] = [
    [123., 104., 238.],
    [240., 128., 128.],
    [143., 188., 143.],
    [222., 184., 135.],
    [255., 127., 80.],
    [128., 128., 128.],
    [107., 142., 35.],
    [100., 100., 100.],
    [52., 231., 123.],
    [55., 19., 112.],
    [255., 255., 150.],
    [202., 62., 94.],
    [205., 145., 63.],
    [183., 118., 231.],
    [110., 251., 201.],
    [175., 155., 50.],
    [105., 205., 48.],
    [37., 70., 25.],
    [121., 33., 135.],
    [83., 140., 208.],
    [0., 154., 37.],
    [178., 220., 205.],
    [255., 152., 213.],
    [200., 90., 174.],
    [175., 200., 74.],
    [63., 25., 12.],
    [87., 87., 87.],
    [129., 215., 234.],
    [102., 242., 126.],
    [29., 105., 20.],
    [129., 74., 25.],
    [129., 38., 192.],
    [160., 160., 160.],
    [129., 197., 122.],
    [157., 175., 255.],
    [41., 208., 208.],
];
pub(super) fn chain(name: &str) -> (String, bool) {
    let name = name.to_lowercase();
    if name.len() == 1 {
        if let Some(index) = "abcdefghijklmnopqrstuvwxyz1234567890".find(&name) {
            return (hex(CHAINS[index]), true);
        }
    }
    let mut hash = 0x811c9dc5_u32;
    for ch in name.chars() {
        let mut utf = [0_u16; 2];
        let first = ch.encode_utf16(&mut utf)[0];
        hash = (hash ^ u32::from(first)).wrapping_mul(0x01000193);
    }
    (
        hex([
            128. + f64::from(hash & 127),
            128. + f64::from((hash >> 7) & 127),
            128. + f64::from((hash >> 14) & 127),
        ]),
        false,
    )
}
pub(super) fn model(number: u32) -> (String, bool) {
    let names = [
        "tan",
        "skyblue",
        "plum",
        "lightgreen",
        "salmon",
        "lightgray",
        "deeppink",
        "gold",
        "dodgerblue",
        "purple",
    ];
    (
        colors::tables().named[names[(number as usize - 1) % names.len()]].clone(),
        number <= 10 && number > 0,
    )
}
const PALETTES: &[(&str, &[&str])] = &[
    (
        "rainbow",
        &["#0000ff", "#00ffff", "#00ff00", "#ffff00", "#ff0000"],
    ),
    ("grayscale", &["#000000", "#ffffff"]),
    ("gray", &["#000000", "#ffffff"]),
    ("red-white-blue", &["#ff0000", "#ffffff", "#0000ff"]),
    ("redblue", &["#ff0000", "#ffffff", "#0000ff"]),
    ("blue-white-red", &["#0000ff", "#ffffff", "#ff0000"]),
    ("bluered", &["#0000ff", "#ffffff", "#ff0000"]),
    ("cyan-white-maroon", &["#0fc7cf", "#ffffff", "#9e205e"]),
    ("cyanmaroon", &["#0fc7cf", "#ffffff", "#9e205e"]),
    ("cyan-gray-maroon", &["#0fc7cf", "#b3b3b3", "#9e205e"]),
    ("lipophilicity", &["#008b8b", "#ffffff", "#b8860b"]),
];
impl Palette {
    pub fn read(text: &str) -> Result<Self, String> {
        let reversed = text.starts_with('^');
        let body = text.strip_prefix('^').unwrap_or(text);
        let mut stops = if !body.contains(':') {
            let known = PALETTES
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(body))
                .ok_or_else(|| {
                    format!(
                        "\"{body}\" is not a palette ({}, or c1:c2:...)",
                        PALETTES
                            .iter()
                            .map(|(n, _)| *n)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?;
            known
                .1
                .iter()
                .enumerate()
                .map(|(i, h)| (i as f64 / (known.1.len() - 1).max(1) as f64, rgb(h)))
                .collect::<Vec<_>>()
        } else {
            let read = |text: &str| {
                colors::read(text)
                    .filter(|c| c.rest.trim().is_empty())
                    .map(|c| rgb(&c.hex))
            };
            let mut parsed = Vec::new();
            for part in body.split(':') {
                if let Some(color) = read(part) {
                    parsed.push((None, color));
                } else if let Some((position, color)) =
                    part.split_once(',').filter(|(p, _)| !p.is_empty())
                {
                    let color = read(color).ok_or_else(|| format!("\"{part}\" is not a colour"))?;
                    parsed.push((
                        Some(position.trim().parse::<f64>().unwrap_or(f64::NAN)),
                        color,
                    ));
                } else {
                    return Err(format!("\"{part}\" is not a colour"));
                }
            }
            let given = parsed.iter().filter(|(p, _)| p.is_some()).count();
            if given != 0 && given != parsed.len() {
                return Err("give a position for every colour of the palette or for none".into());
            }
            let count = parsed.len();
            parsed
                .into_iter()
                .enumerate()
                .map(|(i, (p, c))| (p.unwrap_or(i as f64 / (count - 1) as f64), c))
                .collect()
        };
        if stops
            .iter()
            .enumerate()
            .any(|(i, (p, _))| p.is_nan() || (i > 0 && *p < stops[i - 1].0))
        {
            return Err("palette positions must be numbers in increasing order".into());
        }
        if reversed {
            for (p, _) in &mut stops {
                *p = 1. - *p;
            }
            stops.reverse();
        }
        Ok(Self(stops))
    }
    pub fn color(&self, f: f64) -> String {
        let s = &self.0;
        if f <= s[0].0 {
            return hex(s[0].1);
        }
        for pair in s.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if f <= b.0 {
                let delta = b.0 - a.0;
                let t = (f - a.0) / if delta == 0. { 1. } else { delta };
                return hex(std::array::from_fn(|i| a.1[i] + (b.1[i] - a.1[i]) * t));
            }
        }
        hex(s.last().unwrap().1)
    }
    pub fn colors(&self, count: usize) -> Vec<String> {
        (0..count)
            .map(|i| self.color(i as f64 / (count - 1).max(1) as f64))
            .collect()
    }
    pub fn ranges(&self, residues: &[String]) -> Vec<(String, String, String)> {
        let mut out: Vec<(String, String, String)> = Vec::new();
        let mut previous = None;
        for (i, residue) in residues.iter().enumerate() {
            let step = (i as f64 / (residues.len() - 1).max(1) as f64 * 63.).round() as u32;
            if previous == Some(step) {
                out.last_mut().unwrap().1 = residue.clone();
            } else {
                out.push((
                    residue.clone(),
                    residue.clone(),
                    self.color(f64::from(step) / 63.),
                ));
            }
            previous = Some(step);
        }
        out
    }
}
pub(super) fn random() -> Result<String, String> {
    let mut bytes = [0_u8; 3];
    #[cfg(not(target_arch = "wasm32"))]
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    #[cfg(target_arch = "wasm32")]
    for byte in &mut bytes {
        *byte = (js_sys::Math::random() * 256.).floor() as u8;
    }
    Ok(hex(bytes.map(f64::from)))
}
