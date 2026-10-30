use super::*;
use cosmic_text::{
    fontdb, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight,
    Wrap,
};
use std::sync::{Arc, Mutex, OnceLock};

struct NativeFonts {
    system: FontSystem,
    cache: SwashCache,
}

static FONTS: OnceLock<Mutex<NativeFonts>> = OnceLock::new();

pub(super) fn font_families() -> Vec<String> {
    static FAMILIES: OnceLock<Vec<String>> = OnceLock::new();
    FAMILIES
        .get_or_init(|| {
            FONTS
                .get_or_init(|| Mutex::new(NativeFonts::new()))
                .lock()
                .map(|fonts| {
                    fonts
                        .system
                        .db()
                        .faces()
                        .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
                        .collect()
                })
                .unwrap_or_default()
        })
        .clone()
}

impl NativeFonts {
    fn new() -> Self {
        let mut system = FontSystem::new_with_fonts(
            [
                epaint_default_fonts::HACK_REGULAR,
                epaint_default_fonts::UBUNTU_LIGHT,
            ]
            .map(|bytes| fontdb::Source::Binary(Arc::new(bytes.to_vec()))),
        );
        let choose = |names: &[&str]| {
            names
                .iter()
                .find(|name| {
                    system
                        .db()
                        .query(&fontdb::Query {
                            families: &[Family::Name(name)],
                            ..Default::default()
                        })
                        .is_some()
                })
                .map(|name| name.to_string())
        };
        let sans = choose(&[
            "Arial",
            "Helvetica",
            "DejaVu Sans",
            "Liberation Sans",
            "Ubuntu",
        ]);
        let serif = choose(&[
            "Times New Roman",
            "Times",
            "DejaVu Serif",
            "Liberation Serif",
        ]);
        let mono = choose(&[
            "Courier New",
            "Courier",
            "DejaVu Sans Mono",
            "Liberation Mono",
            "Hack",
        ]);
        if let Some(name) = sans {
            system.db_mut().set_sans_serif_family(name);
        }
        if let Some(name) = serif {
            system.db_mut().set_serif_family(name);
        }
        if let Some(name) = mono {
            system.db_mut().set_monospace_family(name);
        }
        Self {
            system,
            cache: SwashCache::new(),
        }
    }
}

pub(super) fn rasterize(label: &GuiAtomLabel, scale: f32, limit: u32) -> Option<LabelImage> {
    let mut fonts = FONTS
        .get_or_init(|| Mutex::new(NativeFonts::new()))
        .lock()
        .ok()?;
    rasterize_with_fonts(label, scale, limit, &mut fonts)
}

fn rasterize_with_fonts(
    label: &GuiAtomLabel,
    scale: f32,
    limit: u32,
    fonts: &mut NativeFonts,
) -> Option<LabelImage> {
    let NativeFonts { system, cache } = fonts;
    let size = pixel_size(label, scale);
    let style = match label.font_style.to_ascii_lowercase().as_str() {
        "italic" => Style::Italic,
        "oblique" => Style::Oblique,
        _ => Style::Normal,
    };
    let weight = Weight(font_weight(label));
    let names = label
        .font_name
        .split(',')
        .map(|name| name.trim().trim_matches(['\'', '"']));
    let families = names
        .map(|name| match name.to_ascii_lowercase().as_str() {
            "serif" => Family::Serif,
            "sans-serif" => Family::SansSerif,
            "monospace" => Family::Monospace,
            "cursive" => Family::Cursive,
            "fantasy" => Family::Fantasy,
            _ => Family::Name(name),
        })
        .collect::<Vec<_>>();
    let family = system
        .db()
        .query(&fontdb::Query {
            families: &families,
            weight,
            style,
            ..Default::default()
        })
        .and_then(|id| system.db().face(id))
        .and_then(|face| face.families.first())
        .map(|(name, _)| name.clone());
    let attrs = Attrs::new()
        .family(
            family
                .as_deref()
                .map(Family::Name)
                .unwrap_or(Family::SansSerif),
        )
        .weight(weight)
        .style(style);
    let mut buffer = Buffer::new(system, Metrics::new(size.floor(), size.ceil()));
    buffer.set_wrap(Wrap::None);
    buffer.set_size(None, None);
    buffer.set_text(&label.text, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(system, false);
    let logical_width = buffer
        .layout_runs()
        .map(|run| run.line_w)
        .fold(0., f32::max);
    let logical_width = (logical_width.ceil() / 4.).ceil() * 4.;
    let line_count = buffer.lines.len();
    let logical_height = size.ceil() * line_count as f32;
    let width = logical_width.clamp(1., limit as f32) as u32;
    let height = logical_height.clamp(1., limit as f32) as u32;
    let descents = (0..line_count)
        .map(|line| {
            buffer
                .line_layout(system, line)
                .and_then(|lines| lines.first())
                .map_or(0., |line| line.max_descent)
        })
        .collect::<Vec<_>>();
    let mut alpha = vec![0; width as usize * height as usize];
    for run in buffer.layout_runs() {
        let baseline = (run.line_i + 1) as f32 * size.ceil() - descents[run.line_i];
        for glyph in run.glyphs {
            let glyph = glyph.physical((0., baseline), 1.);
            let face_weight = system
                .db()
                .face(glyph.cache_key.font_id)
                .map(|face| face.weight.0);
            let synthetic_bold = weight.0 >= 600
                && face_weight.is_some_and(|actual| actual < 600)
                && system
                    .get_font(glyph.cache_key.font_id, weight)
                    .is_some_and(|font| {
                        font.as_swash()
                            .variations()
                            .find_by_tag(u32::from_be_bytes(*b"wght"))
                            .is_none()
                    });
            let stroke = if synthetic_bold {
                (size / 32.).round().max(1.) as i32
            } else {
                0
            };
            cache.with_pixels(
                system,
                glyph.cache_key,
                Color::rgb(255, 255, 255),
                |x, y, color| {
                    let (x, y) = (glyph.x + x, glyph.y + y);
                    for x in x..=x + stroke {
                        if x >= 0 && y >= 0 && x < width as i32 && y < height as i32 {
                            let pixel = &mut alpha[y as usize * width as usize + x as usize];
                            let value = u32::from(color.a());
                            *pixel =
                                (value + (u32::from(*pixel) * (255 - value) + 127) / 255) as u8;
                        }
                    }
                },
            );
        }
    }
    if cache
        .image_cache
        .values()
        .flatten()
        .map(|image| image.data.len())
        .sum::<usize>()
        > 8 * 1024 * 1024
    {
        cache.image_cache.clear();
    }
    Some(LabelImage {
        width,
        height,
        logical_size: [logical_width, logical_height],
        origin: [0., 0.],
        alpha,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regular_fonts() -> NativeFonts {
        let mut db = fontdb::Database::new();
        db.load_font_data(epaint_default_fonts::HACK_REGULAR.to_vec());
        db.set_sans_serif_family("Hack");
        NativeFonts {
            system: FontSystem::new_with_locale_and_db("en-US".into(), db),
            cache: SwashCache::new(),
        }
    }

    #[test]
    fn standalone_regular_face_can_synthesize_requested_bold() {
        let mut fonts = regular_fonts();
        let mut label = super::super::tests::label("Glyphs");
        let regular = rasterize_with_fonts(&label, 1., 4096, &mut fonts).unwrap();
        label.font_weight = "bold".into();
        let bold = rasterize_with_fonts(&label, 1., 4096, &mut fonts).unwrap();
        assert_eq!(regular.logical_size, bold.logical_size);
        assert!(
            bold.alpha.iter().map(|&v| u64::from(v)).sum::<u64>()
                > regular.alpha.iter().map(|&v| u64::from(v)).sum::<u64>()
        );
    }

    #[test]
    fn native_line_endings_preserve_multiline_labels() {
        let mut fonts = regular_fonts();
        for variants in [
            &["A\nB", "A\rB", "A\r\nB", "A\n\rB"][..],
            &["A\n\nB", "A\r\n\rB", "A\n\r\nB"][..],
        ] {
            let mut label = super::super::tests::label(variants[0]);
            let expected = rasterize_with_fonts(&label, 1., 4096, &mut fonts).unwrap();
            for &text in &variants[1..] {
                label.text = text.into();
                let actual = rasterize_with_fonts(&label, 1., 4096, &mut fonts).unwrap();
                assert_eq!(actual.logical_size, expected.logical_size, "{text:?}");
                assert_eq!(actual.alpha, expected.alpha, "{text:?}");
            }
        }
    }

    #[test]
    fn standalone_regular_face_can_synthesize_requested_italic() {
        let mut fonts = regular_fonts();
        let mut label = super::super::tests::label("Glyphs");
        let regular = rasterize_with_fonts(&label, 1., 4096, &mut fonts).unwrap();
        label.font_style = "italic".into();
        let italic = rasterize_with_fonts(&label, 1., 4096, &mut fonts).unwrap();
        assert_ne!(italic.alpha, regular.alpha);
    }
}
