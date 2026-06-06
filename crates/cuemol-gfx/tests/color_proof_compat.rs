use cuemol_gfx::parse_color;

#[test]
fn generic_cmyk_color_proof_matches_reference_cases() {
    let cases = [
        ("#FFF", 0xF2F3FA, true),
        ("#777", 0x767578, true),
        ("#00F", 0x345BA0, false),
        ("#F0F", 0xB469A4, false),
    ];
    for (expr, code, in_gamut) in cases {
        let color = parse_color(expr).unwrap();
        assert_eq!(color.device_code_generic_cmyk(), Some(code), "{expr}");
        assert_eq!(color.is_in_gamut_generic_cmyk(), Some(in_gamut), "{expr}");
    }
}
