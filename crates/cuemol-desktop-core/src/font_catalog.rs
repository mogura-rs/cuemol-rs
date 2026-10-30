const GENERIC: &[&str] = &["sans-serif", "serif", "monospace", "cursive", "fantasy"];
const FALLBACK: &[&str] = &[
    "Helvetica",
    "Arial",
    "Times New Roman",
    "Georgia",
    "Courier New",
    "Verdana",
    "Trebuchet MS",
    "Menlo",
    "Monaco",
    "Consolas",
];

struct Catalog {
    generation: u32,
    families: Vec<String>,
}

pub fn families() -> Vec<String> {
    use std::sync::{Mutex, OnceLock};
    static CATALOG: OnceLock<Mutex<Option<Catalog>>> = OnceLock::new();
    let Ok(mut cache) = CATALOG.get_or_init(|| Mutex::new(None)).lock() else {
        return build_families(Vec::new());
    };
    let generation = generation();
    if cache
        .as_ref()
        .is_none_or(|previous| previous.generation != generation)
    {
        #[cfg(feature = "offscreen")]
        let names = crate::labels::font_families();
        #[cfg(not(feature = "offscreen"))]
        let names = Vec::new();
        *cache = Some(Catalog {
            generation,
            families: build_families(names),
        });
    }
    cache.as_ref().unwrap().families.clone()
}

fn build_families(mut names: Vec<String>) -> Vec<String> {
    names.retain(|name| {
        !name.trim().is_empty()
            && !GENERIC
                .iter()
                .any(|generic| generic.eq_ignore_ascii_case(name))
    });
    names.sort_by_cached_key(|name| (name.to_lowercase(), name.clone()));
    names.dedup();
    if names.is_empty() {
        names.extend(FALLBACK.iter().map(|name| name.to_string()));
    }
    let mut result = GENERIC
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    result.extend(names);
    result
}

pub fn generation() -> u32 {
    #[cfg(all(feature = "offscreen", target_arch = "wasm32"))]
    {
        return crate::labels::font_catalog_generation();
    }
    #[cfg(not(all(feature = "offscreen", target_arch = "wasm32")))]
    {
        0
    }
}

pub fn request() {
    #[cfg(all(feature = "offscreen", target_arch = "wasm32"))]
    crate::labels::request_font_catalog();
}

pub fn on_change(callback: impl Fn() + 'static) {
    #[cfg(all(feature = "offscreen", target_arch = "wasm32"))]
    crate::labels::on_font_catalog_change(callback);
    #[cfg(not(all(feature = "offscreen", target_arch = "wasm32")))]
    let _ = callback;
}

pub struct FontPreview {
    pub width: u32,
    pub height: u32,
    pub alpha: Vec<u8>,
}

pub fn css_family(family: &str) -> String {
    use std::fmt::Write;
    if GENERIC
        .iter()
        .any(|generic| generic.eq_ignore_ascii_case(family))
    {
        return family.into();
    }
    let mut value = String::from("\"");
    for character in family.chars() {
        match character {
            '"' | '\\' => {
                value.push('\\');
                value.push(character);
            }
            character if character.is_ascii_control() => {
                let _ = write!(value, "\\{:x} ", character as u32);
            }
            _ => value.push(character),
        }
    }
    value.push('"');
    value
}

pub fn preview(family: &str, text: &str, scale: f32) -> Option<FontPreview> {
    #[cfg(feature = "offscreen")]
    {
        let image = crate::labels::rasterize_label(
            &crate::GuiAtomLabel {
                x: 0.,
                y: 0.,
                text: text.chars().take(96).collect(),
                color: "white".into(),
                font_size: 14.,
                font_name: family.into(),
                font_style: "normal".into(),
                font_weight: "normal".into(),
                anchor: [0., 0.],
                depth: 0.,
                on_top: true,
                clip: None,
            },
            scale,
        );
        Some(FontPreview {
            width: image.width,
            height: image.height,
            alpha: image.alpha,
        })
    }
    #[cfg(not(feature = "offscreen"))]
    {
        let _ = (family, text, scale);
        None
    }
}

#[cfg(all(test, feature = "offscreen", not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn installed_and_embedded_families_use_the_label_rasterizer() {
        let names = families();
        assert_eq!(&names[..GENERIC.len()], GENERIC);
        for family in ["Hack", "Ubuntu"] {
            assert!(names.iter().any(|name| name == family), "{family}");
        }
        let mono = preview("Hack", "iiii WWWW", 2.).unwrap();
        let proportional = preview("Ubuntu", "iiii WWWW", 2.).unwrap();
        assert_eq!(mono.alpha.len(), (mono.width * mono.height) as usize);
        assert!(mono.alpha.iter().any(|alpha| *alpha > 0));
        assert_ne!(
            (mono.width, mono.alpha),
            (proportional.width, proportional.alpha)
        );
    }
}

#[cfg(test)]
mod css_tests {
    #[test]
    fn css_faces_preserve_generic_families_and_quote_literal_names() {
        use super::css_family;
        assert_eq!(css_family("serif"), "serif");
        assert_eq!(css_family("8514oem"), "\"8514oem\"");
        assert_eq!(css_family("Times New Roman"), "\"Times New Roman\"");
        assert_eq!(css_family("A\"\\\nB"), "\"A\\\"\\\\\\a B\"");
    }
}
