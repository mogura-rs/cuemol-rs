use cuemol_core::Uid;
use cuemol_desktop_core::{DesktopAppState, GuiLoadedPath, GuiStyleTarget};

const PDB: &[u8] =
    b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n";

fn load(app: &mut DesktopAppState) -> Uid {
    match app.open_bytes("molecule.pdb", PDB).unwrap() {
        GuiLoadedPath::Object { object_uid, .. } => object_uid,
        other => panic!("expected molecule, got {other:?}"),
    }
}

fn user_style(app: &DesktopAppState) -> GuiStyleTarget {
    app.style_summaries(Uid::new(0))
        .into_iter()
        .find(|style| style.name == "user")
        .unwrap()
        .target
}

fn rgb(app: &DesktopAppState, scope: Uid, name: &str) -> [u8; 4] {
    let color = app.resolve_style_color(scope, name).unwrap();
    [color.r(), color.g(), color.b(), color.a()]
}

#[test]
fn current_default_palette_uses_molecule_order_and_one_portable_scene_style() {
    let mut app = DesktopAppState::new();
    for index in 0..8 {
        let object = load(&mut app);
        let base = format!("okabeito_{}", index % 7 + 1);
        let paint = app.scene_manager().object(object).unwrap().paint_coloring();
        assert_eq!(
            paint,
            [
                ("sheet".into(), base.clone()),
                ("helix".into(), base.clone()),
                ("nucleic".into(), format!("{base}{{mod_h:15}}")),
                ("*".into(), format!("{base}{{mod_s:-0.6;mod_b:0.1}}")),
            ]
        );
    }
    let scene = app.active_scene().unwrap();
    let local = app
        .style_summaries(scene)
        .into_iter()
        .filter(|style| style.local_index.is_some())
        .collect::<Vec<_>>();
    assert_eq!(local.len(), 1);
    assert_eq!(local[0].name, "default");
    assert_eq!(rgb(&app, scene, "okabeito_1"), [230, 159, 0, 255]);
    app.edit_style_entry(user_style(&app), "color", "okabeito_1", Some("#ff0000"))
        .unwrap();
    assert_eq!(rgb(&app, Uid::new(0), "okabeito_1"), [255, 0, 0, 255]);
    assert_eq!(rgb(&app, scene, "okabeito_1"), [230, 159, 0, 255]);
    let bytes = app.active_scene_bytes().unwrap();
    let mut restored = DesktopAppState::new();
    restored
        .edit_style_entry(
            user_style(&restored),
            "color",
            "okabeito_1",
            Some("#00ff00"),
        )
        .unwrap();
    restored.open_bytes("colors.qsc", &bytes).unwrap();
    assert_eq!(
        rgb(&restored, restored.active_scene().unwrap(), "okabeito_1"),
        [230, 159, 0, 255]
    );
}

#[test]
fn custom_global_palettes_use_numeric_order_and_keep_existing_scene_colors() {
    let mut app = DesktopAppState::new();
    app.edit_style(
        user_style(&app),
        r##"<styles id="user">
        <palette id="default_paint" value="custom-colors"/>
        <color id="customcolors_10" value="#abcdef"/>
        <color id="customcolors_2" value="#123456{alpha:0.5}"/>
        </styles>"##,
    )
    .unwrap();
    let scene = app.ensure_active_scene();
    app.import_scene_style(
        scene,
        r##"<styles id="default"><color id="customcolors_10" value="#fedcba"/></styles>"##,
        true,
    )
    .unwrap();
    for expected in ["customcolors_2", "customcolors_10", "customcolors_2"] {
        let object = load(&mut app);
        assert_eq!(
            app.scene_manager().object(object).unwrap().paint_coloring()[0].1,
            expected
        );
    }
    assert_eq!(rgb(&app, scene, "customcolors_2"), [18, 52, 86, 255]);
    assert_eq!(rgb(&app, scene, "customcolors_10"), [254, 220, 186, 255]);
    let local = app
        .style_summaries(scene)
        .into_iter()
        .filter(|style| style.local_index.is_some())
        .collect::<Vec<_>>();
    assert_eq!(local.len(), 2);
    assert_eq!(
        local.iter().find(|style| !style.read_only).unwrap().name,
        "default_0"
    );
}

#[test]
fn unknown_palette_falls_back_to_current_reference_theme() {
    for theme in ["", "missing"] {
        let mut app = DesktopAppState::new();
        app.edit_style(
            user_style(&app),
            &format!(r#"<styles><palette id="default_paint" value="{theme}"/></styles>"#),
        )
        .unwrap();
        let object = load(&mut app);
        assert_eq!(
            app.scene_manager().object(object).unwrap().paint_coloring()[0].1,
            "okabeito_1"
        );
    }
}

#[test]
fn optional_reference_palette_algorithm_matches_custom_and_current_themes() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let Ok(reference) = std::env::var("CUEMOL_DEFAULT_PAINT_REFERENCE") else {
        return;
    };
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/check-reference-default-paint.mjs");
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    for theme in [
        "okabeito",
        "tolbright",
        "tolvibrant",
        "tolmuted",
        "tollight",
        "brewerset2",
        "brewerdark2",
        "tableau10",
        "custom-colors",
        "missing",
        "",
    ] {
        let mut app = DesktopAppState::new();
        app.edit_style(user_style(&app), &format!(r##"<styles id="user"><palette id="default_paint" value="{theme}"/><color id="customcolors_2" value="#123456"/><color id="customcolors_10" value="#abcdef"/></styles>"##)).unwrap();
        let global = cuemol_molvis::default_style_manager();
        let mut colors = serde_json::Map::new();
        for name in global.color_names(0).into_iter().filter(|name| {
            name.rsplit_once('_')
                .is_some_and(|(_, suffix)| suffix.parse::<u32>().is_ok())
        }) {
            let color = rgb(&app, Uid::new(0), &name);
            colors.insert(name, serde_json::json!([color[0], color[1], color[2]]));
        }
        colors.insert("customcolors_2".into(), serde_json::json!([18, 52, 86]));
        colors.insert("customcolors_10".into(), serde_json::json!([171, 205, 239]));
        let mut objects = Vec::new();
        let mut local = serde_json::Map::new();
        for _ in 0..12 {
            let target = load(&mut app);
            let scene = app.active_scene().unwrap();
            objects.push(serde_json::json!({"uid":target.get(),"className":"MolCoord"}));
            cases.push(serde_json::json!({"theme":theme,"objects":objects,"target":target.get(),"colors":colors,"local":local}));
            let paint = app.scene_manager().object(target).unwrap().paint_coloring();
            let base = &paint[0].1;
            let created = if local.contains_key(base) {
                Vec::new()
            } else {
                let c = rgb(&app, scene, base);
                let value = format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
                local.insert(base.clone(), serde_json::json!(value));
                vec![vec![base.clone(), value]]
            };
            expected.push(serde_json::json!({"paint":paint,"created":created}));
        }
    }
    let mut process = Command::new("node")
        .arg(script)
        .arg(reference)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    process
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(&cases).unwrap().as_bytes())
        .unwrap();
    let output = process.wait_with_output().unwrap();
    assert!(output.status.success());
    let observed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(observed, serde_json::json!(expected));
}

#[cfg(feature = "scripting")]
#[test]
fn script_paint_keeps_named_colors_and_resolves_live_scene_changes() {
    let mut app = DesktopAppState::new();
    let uid = load(&mut app);
    let (log, _) = app.run_script_js(&format!(r##"
        var mol=cuemol.getObject({});
        var palette=cuemol.getService('StyleManager');
        var set=palette.createStyleSet('paint-test',scene.uid);
        palette.setColor('accent',cuemol.makeColor('#ff0000'),scene.uid,set);
        palette.firePendingEvents();
        var paint=cuemol.PaintColoring();
        paint.append(cuemol.makeSel('*'),cuemol.makeColor('accent{{mod_b:-0.5}}',scene.uid));
        mol.coloring=paint;
        var retained=mol.coloring.getColorAt(0);
        print(retained.type+':'+retained.toString()+':'+retained.r());
        var renderer=mol.createRenderer('cpk');renderer.coloring=cuemol.PaintColoring();
        renderer.coloring.append(cuemol.makeSel('*'),retained);
        var rendered=renderer.coloring.getColorAt(0);
        print(rendered.type+':'+rendered.toString()+':'+rendered.r());
        palette.setColor('accent',cuemol.makeColor('#00ff00'),scene.uid,set);
        palette.firePendingEvents();
        print(retained.r()+','+retained.g()+':'+rendered.r()+','+rendered.g());
        mol.coloring.insertBefore(0,cuemol.makeSel('elem C'),rendered);
        renderer.coloring.changeAt(0,cuemol.makeSel('*'),cuemol.makeColor('accent{{alpha:0.25}}',scene.uid));
        print(mol.coloring.getColorAt(0).toString()+':'+renderer.coloring.getColorAt(0).toString());
    "##,uid.get())).unwrap();
    assert_eq!(log,"NamedColor:accent{mod_b:-0.5}:128\nNamedColor:accent{mod_b:-0.5}:128\n0,128:0,128\naccent{mod_b:-0.5}:accent{alpha:0.25}\n");
    let paint = app.scene_manager().object(uid).unwrap().paint_coloring();
    assert!(paint.iter().all(|(_, color)| color == "accent{mod_b:-0.5}"));
    let data = String::from_utf8(app.active_scene_bytes().unwrap()).unwrap();
    assert!(data.contains("accent{mod_b:-0.5}"));
    assert!(data.contains("accent{alpha:0.25}"));
}
