#![cfg(feature = "scripting")]
use cuemol_core::Uid;
use cuemol_desktop_core::{
    picking::{PickImage, PickRecord, PickStamp},
    DesktopAppState,
};
use cuemol_qsys::{Camera, ViewHoverHit};
use std::sync::Arc;

#[test]
fn hover_service_reset_and_receiver_scoped_pick_images() {
    let mut app = DesktopAppState::new();
    let created = app.new_scene("hover", true);
    let view = created.result_view.unwrap();
    let data = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        data.path(),
        "HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n",
    )
    .unwrap();
    let object = app
        .load_object(created.result_scene, data.path(), "target", "pdb")
        .unwrap()
        .result_object;
    let renderer = app
        .new_renderer(created.result_scene, object, "cpk", "spheres")
        .unwrap()
        .result_renderer;
    let log = app.run_script_js(&format!("var v=scene.getView({}),input=getService('ViewInputConfig');input.gpu_pick=false;input.hover_hl_color=cuemol.makeColor('blue');input.hitprec=2;cuemol.resetProp(input,'gpu_pick');cuemol.resetProp(input,'hover_hl_color');cuemol.resetProp(input,'hitprec');print(input.gpu_pick+':'+input.hover_hl_color.r()+':'+input.hover_hl_color.g()+':'+input.hitprec);v.setHoverHit({},1,-1);", view.get(), renderer.get())).unwrap().0;
    assert_eq!(log, "true:255:102:10\n");
    let hit = ViewHoverHit {
        renderer_id: renderer,
        atom_id: 1,
        symm_id: -1,
    };
    assert_eq!(app.active_hover_hit(), Some(hit));
    app.run_script_js(&format!(
        "scene.getView({}).setHoverHit({},-1,0);",
        view.get(),
        renderer.get()
    ))
    .unwrap();
    assert_eq!(app.active_hover_hit(), None);

    let mut camera = Camera::new();
    camera.set_zoom(10.);
    app.sync_viewport_pick_image(
        &camera,
        [100., 100.],
        Some(Arc::new(PickImage {
            stamp: PickStamp {
                revision: 1,
                matrix: [[0.; 4]; 4],
                size: [100, 100],
            },
            size: [1, 1],
            pixels: vec![[1, 0, 0, 0]],
            records: vec![PickRecord {
                hit,
                object_id: object.get(),
                atom_index: 0,
            }]
            .into(),
        })),
    );
    let log = app.run_script_js(&format!("var v=scene.getView({});var h=JSON.parse(v.hitTest(10,10));print(h.atom_id+':'+h.rend_name);var other=scene.createView();print(other.hitTest(10,10)==='');v.zoom=15;print(v.hitTest(10,10)==='');",view.get())).unwrap().0;
    assert_eq!(log, "1:spheres\ntrue\ntrue\n");
    let log = app.run_script_js(&format!("var v=scene.getView({});getService('ViewInputConfig').gpu_pick=false;print(JSON.parse(v.hitTest(50,50)).atom_id);print(JSON.parse(v.hitTestRect(40,40,20,20,false)).length);",view.get())).unwrap().0;
    assert_eq!(log, "1\n1\n");
    assert_ne!(renderer, Uid::new(0));
}
