//! Regression fixtures for ICC soft proofing, recorded from LittleCMS.

use super::CmsXform;
use std::path::{Path, PathBuf};

const STEP: usize = 15;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn reference_cmyk() -> Option<Vec<u8>> {
    let reference = std::env::var_os("CUEMOL2_REF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
    std::fs::read(reference.join("src/xul_gui/data/icc_profiles/GenericCMYK.icm")).ok()
}

fn samples() -> impl Iterator<Item = u32> {
    (0..=255).step_by(STEP).flat_map(|r| {
        (0..=255).step_by(STEP).flat_map(move |g| {
            (0..=255)
                .step_by(STEP)
                .map(move |b| (r << 16) | (g << 8) | b)
        })
    })
}

fn render(profile: &[u8]) -> String {
    let mut out = String::new();
    for intent in 0..4 {
        let mut transform = CmsXform::new();
        transform.set_icc_intent(intent);
        transform.load_icc_bytes(profile).unwrap();
        if intent == 0 {
            out.push_str(&format!("info {:?}\n", transform.info()));
        }
        out.push_str(&format!("intent {intent} "));
        for rgb in samples() {
            let code = transform.do_xform(0xff00_0000 | rgb) & 0x00ff_ffff;
            let gamut = transform.is_in_gamut(0xff00_0000 | rgb);
            out.push_str(&format!("{code:06x}{}", u8::from(gamut)));
        }
        out.push('\n');
    }
    out
}

fn profiles() -> Vec<(&'static str, Option<Vec<u8>>)> {
    vec![
        (
            "srgb",
            std::fs::read(golden_dir().join("srgb_lcms.icc")).ok(),
        ),
        ("generic_cmyk", reference_cmyk()),
    ]
}

/// Records the fixtures; run with LittleCMS still linked.
#[test]
#[ignore]
fn record_cms_golden() {
    std::fs::create_dir_all(golden_dir()).unwrap();
    std::fs::write(
        golden_dir().join("srgb_lcms.icc"),
        super::tests::srgb_profile(),
    )
    .unwrap();
    for (name, profile) in profiles() {
        let profile = profile.expect("profile");
        std::fs::write(
            golden_dir().join(format!("cms_{name}.txt")),
            render(&profile),
        )
        .unwrap();
    }
}

#[test]
fn soft_proofing_matches_golden() {
    for (name, profile) in profiles() {
        let Some(profile) = profile else {
            continue;
        };
        let expected =
            std::fs::read_to_string(golden_dir().join(format!("cms_{name}.txt"))).unwrap();
        let actual = render(&profile);
        for (line, (e, a)) in expected.lines().zip(actual.lines()).enumerate() {
            if line == 0 {
                assert_eq!(e, a, "{name}: info");
                continue;
            }
            let (e, a) = (e.as_bytes(), a.as_bytes());
            assert_eq!(e.len(), a.len(), "{name}: line {line}");
            let header = "intent 0 ".len();
            for (index, (ec, ac)) in e[header..].chunks(7).zip(a[header..].chunks(7)).enumerate() {
                assert_eq!(
                    ec,
                    ac,
                    "{name}: intent {} sample {index} ({:06x}): {} vs {}",
                    line - 1,
                    samples().nth(index).unwrap(),
                    std::str::from_utf8(ec).unwrap(),
                    std::str::from_utf8(ac).unwrap()
                );
            }
        }
    }
}
