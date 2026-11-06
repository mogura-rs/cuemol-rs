#![cfg(all(feature = "scripting", feature = "offscreen"))]

use cuemol_desktop_core::DesktopAppState;

#[test]
fn script_pov_exports_live_label_images_and_clears_disabled_pixel_state() {
    let mut app = DesktopAppState::new();
    app.open_bytes(
        "labels.pdb",
        b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n",
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let pov = directory.path().join("labels.pov");
    let inc = directory.path().join("labels.inc");
    let source = format!(
        "var mol=cuemol.getObject({}); \
         var label=mol.createRenderer('*namelabel'); \
         label.setLabel(1,'label'); label.color=cuemol.makeColor('#00ff00'); \
         var e=cuemol.PovSceneExporter(); e.attach(scene); \
         e.setPath({}); e.setSubPath('inc',{}); \
         e.width=96; e.height=96; e.usePixImgs=true; e.perspective=false; \
         e.write(); print(e.imgFileNames); \
         label.color=cuemol.makeColor('#ff0000'); e.perspective=true; \
         e.write(); print(e.imgFileNames); \
         e.usePixImgs=false; e.write(); print(\'files=\'+e.imgFileNames);",
        app.active_object_summaries()[0].uid.get(),
        serde_json::to_string(&pov.to_string_lossy()).unwrap(),
        serde_json::to_string(&inc.to_string_lossy()).unwrap(),
    );
    let (log, _) = app.run_script_js(&source).unwrap();
    let images: Vec<_> = log.lines().collect();
    assert_eq!(images.len(), 3);
    assert_eq!(images[2], "files=");
    assert_eq!(images[0], images[1]);
    assert!(!images[0].is_empty());
    let mut png = png::Decoder::new(std::fs::File::open(images[0]).unwrap())
        .read_info()
        .unwrap();
    let mut pixels = vec![0; png.output_buffer_size()];
    png.next_frame(&mut pixels).unwrap();
    assert_eq!(png.info().color_type, png::ColorType::Rgba);
    assert!(pixels
        .chunks_exact(4)
        .any(|pixel| pixel[0] == 255 && pixel[1] == 0 && pixel[3] > 0));
    assert!(!std::fs::read_to_string(inc)
        .unwrap()
        .contains("// Pixel image"));
}
