use super::*;

fn export_scene() -> (DesktopAppState, Uid) {
    let mut app = DesktopAppState::new();
    let scene = app.new_scene("exports", true).result_scene;
    let molecule = read_pdb_str(
        "ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n",
    )
    .unwrap();
    let object = app
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("mol", molecule);
    app.new_renderer(scene, object, "cpk", "atoms").unwrap();
    let mut camera = Camera::new();
    camera.set_name("fixed");
    app.scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .set_camera(camera);
    (app, object)
}

fn stl_mean_x(path: &std::path::Path) -> f64 {
    let data = fs::read(path).unwrap();
    let count = u32::from_le_bytes(data[80..84].try_into().unwrap()) as usize;
    assert!(count > 0);
    assert_eq!(data.len(), 84 + count * 50);
    data[84..]
        .chunks_exact(50)
        .flat_map(|face| {
            [12, 24, 36].map(|i| f32::from_le_bytes(face[i..i + 4].try_into().unwrap()) as f64)
        })
        .sum::<f64>()
        / (count * 3) as f64
}

#[test]
fn native_exporters_use_edits_made_in_the_current_script() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, object) = export_scene();
    let first = dir.path().join("before.stl");
    let second = dir.path().join("after.stl");
    app.run_script_js(&format!(
        "var writer = cuemol.StlSceneExporter(); writer.attach(scene); \
         writer.camera = 'fixed'; writer.scale = 1; writer.setPath({first:?}); writer.write(); \
         var atom = cuemol.getObject({object}).getAtomByID(1); \
         atom.pos = cuemol.Vector('(12,0,0,0)'); \
         writer.setPath({second:?}); writer.write();",
        first = first.to_string_lossy(),
        second = second.to_string_lossy(),
        object = object.get(),
    ))
    .unwrap();
    assert!((stl_mean_x(&second) - stl_mean_x(&first) - 12.0).abs() < 1e-5);
}

#[test]
fn animation_frames_write_the_supplied_exporter_with_current_geometry_and_camera() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = export_scene();
    let paths: Vec<_> = (0..3)
        .map(|i| dir.path().join(format!("frame-{i}.stl")))
        .collect();
    let (log, _) = app.run_script_js(&format!(
        "function time(ms) {{ var t = cuemol.TimeValue(); t.millisec = ms; return t; }} \
         var manager = scene.getAnimMgr(); manager.startcam = 'fixed'; \
         var destination = cuemol.Camera(); destination.center = cuemol.Vector('(4,0,0,0)'); \
         scene.setCamera('destination', destination); \
         var move = cuemol.CamMotion(); move.endcam = 'destination'; move.end = time(1000); \
         manager.append(move); var shift = cuemol.RendXformAnim(); shift.rend = 'mol/atoms'; \
         shift.end_tran = cuemol.Vector('(10,0,0,0)'); shift.end = time(1000); manager.append(shift); \
         var writer = cuemol.StlSceneExporter(); writer.scale = 1; writer.camera = 'fixed'; \
         print('count=' + manager.setupRender(time(0), time(1000), 2)); \
         writer.setPath({first:?}); manager.writeFrame(writer); \
         print('first=' + manager.frameno + ':' + (writer.detach() === null)); \
         writer.setPath({second:?}); print('begin=' + manager.beginFrame(writer)); \
         print('mid=' + manager.elapsed.millisec); writer.write(); manager.endFrame(writer); \
         writer.setPath({third:?}); manager.writeFrame(writer); \
         print('end=' + manager.frameno + ':' + manager.beginFrame(writer) + ':' + \
               (writer.detach() === null) + ':' + writer.camera); \
         try {{ manager.beginFrame({{}}); print('invalid=false'); }} \
         catch (e) {{ print('invalid=true'); }}",
        first=paths[0].to_string_lossy(), second=paths[1].to_string_lossy(), third=paths[2].to_string_lossy(),
    )).unwrap();
    assert_eq!(
        log,
        "count=3\nfirst=1:true\nbegin=true\nmid=500\nend=3:false:true:fixed\ninvalid=true\n"
    );
    let first = stl_mean_x(&paths[0]);
    for (i, path) in paths.iter().enumerate() {
        let offset = stl_mean_x(path) - first;
        assert!(
            (offset - i as f64 * 3.0).abs() < 1e-5,
            "frame {i}: {offset}"
        );
    }
}

#[test]
fn failed_frame_write_detaches_the_exporter_without_advancing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = export_scene();
    let path = dir.path().join("missing").join("frame.stl");
    let (log, _) = app
        .run_script_js(&format!(
            "var manager = scene.getAnimMgr(); \
         manager.setupRender(cuemol.TimeValue(), cuemol.TimeValue(), 1); \
         var writer = cuemol.StlSceneExporter(); writer.setPath({path:?}); \
         try {{ manager.writeFrame(writer); print('failed=false'); }} \
         catch (e) {{ print('failed=true'); }} \
         print(manager.frameno + ':' + (writer.detach() === null));",
            path = path.to_string_lossy(),
        ))
        .unwrap();
    assert_eq!(log, "failed=true\n0:true\n");
    assert!(!path.exists());
}

#[test]
fn stopping_animation_in_a_later_script_restores_affected_properties() {
    let (mut app, object) = export_scene();
    let view = app.active_view_summary().unwrap().uid;
    app.run_script_js(&format!(
        "function time(ms) {{ var t = cuemol.TimeValue(); t.millisec = ms; return t; }} \
         var renderer = cuemol.getObject({object}).getRendererByName('atoms'); \
         renderer.alpha = 0.8; var original = cuemol.Matrix(); original.setAt(1, 4, 3); \
         renderer.xformMat = original; \
         var manager = scene.getAnimMgr(); \
         var fade = cuemol.RealPropAnim(); fade.rend = 'mol/atoms'; fade.prop = 'alpha'; \
         fade.startValue = 0; fade.endValue = 1; fade.end = time(1000); manager.append(fade); \
         var shift = cuemol.RendXformAnim(); shift.rend = 'mol/atoms'; \
         shift.end_tran = cuemol.Vector('(10,0,0,0)'); shift.end = time(1000); manager.append(shift); \
         manager.goTime(time(500), cuemol.getView({view}));",
        object=object.get(), view=view.get(),
    )).unwrap();
    let (log, _) = app.run_script_js(&format!(
        "var renderer = cuemol.getObject({object}).getRendererByName('atoms'); \
         var manager = scene.getAnimMgr(); \
         print('before=' + renderer.alpha + ':' + renderer.xformMat.getAt(1, 4)); \
         manager.stop(); \
         print('after=' + renderer.alpha + ':' + renderer.xformMat.getAt(1, 4) + ':' + manager.playState);",
        object=object.get(),
    )).unwrap();
    assert_eq!(log, "before=0.5:5\nafter=0.8:3:stop\n");
}

#[test]
fn asynchronous_readers_keep_filters_options_and_jobs_across_script_calls() {
    let (mut app, object) = export_scene();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input");
    let mut output = cuemol_qsys::InOutHandler::new("sdf", "SDF", "*.sdf", 1);
    output.set_path(path.to_string_lossy());
    output.set_compress(1);
    output.set_base64(true);
    output
        .write_string(&cuemol_molstr::write_sdf_string(
            app.scene_manager().object(object).unwrap().mol(),
        ))
        .unwrap();
    let data = fs::read(path).unwrap();
    let (log, _) = app
        .run_script_js(
            "var reader=cuemol.SDFMolReader(); reader.chain_name='X'; reader.resid_index=7; \
         reader.compress='gzip'; reader.base64=true; reader.path='input.sdf.gz.b64'; \
         print(cuemol.getService('StreamManager').loadObjectAsync(reader));",
        )
        .unwrap();
    let id: i32 = log.trim().parse().unwrap();
    for chunk in data.chunks(data.len().div_ceil(2)) {
        app.run_script_js(&format!(
            "var data={chunk:?}; var bytes=cuemol.ByteArray(); bytes.init(bytes.UINT8,data.length); \
             for(var i=0;i<data.length;i++) bytes.setAt(i,data[i]); \
             cuemol.getService('StreamManager').supplyDataAsync({id},bytes,bytes.length);"
        )).unwrap();
    }
    let (log, _) = app.run_script_js(&format!(
        "var streams=cuemol.getService('StreamManager'); var object=streams.waitLoadAsync({id}); \
         object.name='async'; scene.addObject(object); \
         print(object.getAtomCount()+':'+object.src+':'+object.srctype+':'+(streams.waitLoadAsync({id})===null));"
    )).unwrap();
    assert_eq!(log, "1:input.sdf.gz.b64:sdf:true\n");
    let scene = app
        .scene_manager()
        .scene(app.active_scene().unwrap())
        .unwrap();
    let object = scene
        .objects()
        .iter()
        .find(|object| object.name() == "async")
        .unwrap();
    assert_eq!(object.mol().atoms()[0].chain_id, "X");
    assert_eq!(object.mol().atoms()[0].res_id, 7);
}

#[test]
fn asynchronous_readers_return_density_and_surface_objects_and_discard_failed_jobs() {
    let (mut app, _) = export_scene();
    for (class, name, data) in [
        ("OpenDXPotReader", "density", "object 1 class gridpositions counts 2 2 2\norigin 0 0 0\ndelta 1 0 0\ndelta 0 1 0\ndelta 0 0 1\nobject 2 class gridconnections counts 2 2 2\nobject 3 class array type double rank 0 items 8 data follows\n0 1 2 3 4 5 6 7\n"),
        ("PLYFileReader", "surface", "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n0 1 0\n3 0 1 2\n"),
    ] {
        let (log, _) = app.run_script_js(&format!(
            "var reader=cuemol.{class}();reader.path='{name}'; \
             var streams=cuemol.getService('StreamManager');var id=streams.loadObjectAsync(reader); \
             var data={:?};var bytes=cuemol.ByteArray();bytes.init(bytes.UINT8,data.length); \
             for(var i=0;i<data.length;i++)bytes.setAt(i,data[i]); \
             try {{ streams.supplyDataAsync(id,bytes,bytes.length+1); }} catch(e) {{ print('range'); }} \
             streams.supplyDataAsync(id,bytes,bytes.length);var object=streams.waitLoadAsync(id); \
             object.name='{name}';scene.addObject(object);print(object.src);",data.as_bytes()
        )).unwrap();
        assert_eq!(log, format!("range\n{name}\n"));
    }
    let scene = app.active_scene().unwrap();
    assert_eq!(app.density_map_summaries(scene).len(), 1);
    assert_eq!(app.surface_summaries(scene).len(), 1);
    let (log, _) = app
        .run_script_js(
            "var streams=cuemol.getService('StreamManager');var reader=cuemol.SDFMolReader(); \
         reader.compress='gzip';var id=streams.loadObjectAsync(reader); \
         try { streams.waitLoadAsync(id); } catch(e) { print('failed'); } \
         print(streams.waitLoadAsync(id)===null);",
        )
        .unwrap();
    assert_eq!(log, "failed\ntrue\n");
}

#[test]
fn stream_factories_reset_defaults_before_reading_unlike_direct_constructors() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input.sdf");
    let (mut app, object) = export_scene();
    let sdf = cuemol_molstr::write_sdf_string(app.scene_manager().object(object).unwrap().mol());
    fs::write(&path, &sdf).unwrap();
    let (log, _) = app.run_script_js(&format!(
        "var streams = cuemol.getService('StreamManager'); \
         print(cuemol.MmcifMolReader().loadsecstr + ':' + streams.createHandler('mmcif', 0).loadsecstr); \
         print(cuemol.SDFMolReader().chain_name + ':' + streams.createHandler('sdf', 0).chain_name); \
         print(cuemol.MOL2MolReader().chain_name + ':' + streams.createHandler('mol2', 0).chain_name); \
         var reader = streams.createHandler('sdf', 0); var molecule = reader.createDefaultObj(); \
         molecule.name = 'factory'; reader.attach(molecule); reader.setPath({path:?}); reader.read(); \
         scene.addObject(molecule);",
        path = path.to_string_lossy(),
    )).unwrap();
    assert_eq!(log, "true:false\nA:_\nA:_\n");
    let scene = app
        .scene_manager()
        .scene(app.active_scene().unwrap())
        .unwrap();
    let molecule = scene
        .objects()
        .iter()
        .find(|object| object.name() == "factory")
        .unwrap();
    assert_eq!(molecule.mol().atoms()[0].chain_id, "_");
    let document = format!(
        "<scene><object type=\"MolCoord\" name=\"linked\" src=\"datachunk:00000\" srctype=\"sdf\"/></scene>\n\
         ========== End of XML ==========\n==========START datachunk:00000==========\n{sdf}\n\
         ==========END datachunk:00000==========\n"
    );
    let mut reloaded = DesktopAppState::new();
    reloaded
        .open_bytes("scene.qsc", document.as_bytes())
        .unwrap();
    let scene = reloaded
        .scene_manager()
        .scene(reloaded.active_scene().unwrap())
        .unwrap();
    assert_eq!(scene.objects()[0].mol().atoms()[0].chain_id, "_");
}

#[test]
fn scene_handlers_complete_before_the_next_script_statement() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.qsc");
    let (mut app, object) = export_scene();
    let source = app.active_scene().unwrap();
    let target = app.new_scene("target", true).result_scene;
    let (log, _) = app
        .run_script_js(&format!(
            "var source = cuemol.getScene({source}); var target = cuemol.getScene({target}); \
         cuemol.getObject({object}).name = 'written'; \
         var writer = cuemol.SceneXMLWriter(); writer.attach(source); writer.setPath({path:?}); \
         writer.write(); \
         var reader = cuemol.SceneXMLReader(); reader.attach(target); reader.setPath({path:?}); \
         reader.read(); \
         print('read=' + target.getObjectByName('written').name);",
            source = source.get(),
            target = target.get(),
            object = object.get(),
            path = path.to_string_lossy(),
        ))
        .unwrap();
    assert_eq!(log, "read=written\n");
    assert_eq!(app.object_summaries(target)[0].atom_count, 1);
    assert!(path.is_file());
}

#[test]
fn scene_handler_io_failures_are_catchable_at_the_call_site() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing").join("scene.qsc");
    let (mut app, _) = export_scene();
    let (log, _) = app
        .run_script_js(&format!(
        "var reader = cuemol.SceneXMLReader(); reader.attach(scene); reader.setPath({path:?}); \
         try {{ reader.read(); print('read=false'); }} catch (e) {{ print('read=true'); }} \
         var writer = cuemol.SceneXMLWriter(); writer.attach(scene); writer.setPath({path:?}); \
         try {{ writer.write(); print('write=false'); }} catch (e) {{ print('write=true'); }} \
         var command = cuemol.LoadSceneCommand(); command.target_scene = scene; \
         command.file_path = {path:?}; \
         try {{ command.run(); print('command=false'); }} catch (e) {{ print('command=true'); }}",
        path=missing.to_string_lossy(),
    ))
        .unwrap();
    assert_eq!(log, "read=true\nwrite=true\ncommand=true\n");
    assert!(!missing.exists());
}

#[test]
fn sdf_script_writer_filters_atoms_and_remaps_selected_bonds() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("selected.sdf");
    let (mut app, object) = export_scene();
    let mol = app
        .scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .mol_mut();
    let mut atom = mol.atoms()[0].clone();
    atom.serial = 2;
    atom.name = "CB".into();
    atom.position.x = 1.5;
    mol.push_atom(atom.clone());
    atom.serial = 3;
    atom.name = "CG".into();
    atom.position.x = 3.0;
    mol.push_atom(atom);
    mol.add_bond(0, 1, cuemol_molstr::BondOrder::Single);
    mol.add_bond(1, 2, cuemol_molstr::BondOrder::Double);
    app.run_script_js(&format!(
        "var writer = cuemol.SDFMolWriter(); writer.attach(cuemol.getObject({object})); \
         writer.sel = cuemol.SelCommand('name CB,CG'); writer.setPath({path:?}); writer.write();",
        object = object.get(),
        path = path.to_string_lossy(),
    ))
    .unwrap();
    let selected = cuemol_molstr::read_sdf_file(path).unwrap();
    assert_eq!(selected.atom_count(), 2);
    assert_eq!(selected.bond_count(), 1);
    assert_eq!(selected.bonds()[0].order, cuemol_molstr::BondOrder::Double);
    assert_eq!(selected.atoms()[0].position.x, 1.5);
}

#[test]
fn load_object_command_returns_the_object_added_to_the_scene() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.pdb");
    fs::write(
        &path,
        "ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n",
    )
    .unwrap();
    let mut app = DesktopAppState::new();
    let scene = app.new_scene("load", true).result_scene;
    let (log, _) = app
        .run_script_js(&format!(
            "var command = cuemol.LoadObjectCommand(); command.target_scene = scene; \
         command.file_path = {path:?}; command.object_name = 'loaded'; command.run(); \
         var loaded = command.result_object; \
         print('identity=' + (scene.getObjectByName('loaded').uid === loaded.uid)); \
         loaded.getAtomByID(1).pos = cuemol.Vector('(7,0,0,0)'); \
         print('coordinate=' + cuemol.getObject(loaded.uid).getAtomByID(1).pos.x);",
            path = path.to_string_lossy(),
        ))
        .unwrap();
    assert_eq!(log, "identity=true\ncoordinate=7\n");
    let objects = app.scene_manager().scene(scene).unwrap().objects();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].mol().atoms()[0].position.x, 7.0);
    assert_eq!(objects[0].source_type(), "pdb");
    assert_eq!(objects[0].source(), path.to_string_lossy());
}

#[test]
fn script_stream_filters_and_link_metadata_survive_scene_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let gzip = dir.path().join("input.pdb.gz");
    let encoded = dir.path().join("output.sdf.b64");
    let linked = dir.path().join("linked.qsc");
    let embedded = dir.path().join("embedded.qsc");
    let mut stream = cuemol_qsys::InOutHandler::default();
    stream.set_path(gzip.to_string_lossy());
    stream.set_compress(cuemol_qsys::InOutHandler::COMP_GZIP);
    stream.write_string("ATOM      1  CA  GLY A   1       7.000   0.000   0.000  1.00 10.00           C  \nEND\n").unwrap();
    let mut app = DesktopAppState::new();
    app.new_scene("io", true);
    let (log, _) = app
        .run_script_js(&format!(
            "var command = cuemol.LoadObjectCommand(); command.target_scene = scene; \
         command.file_path = {gzip:?}; command.run(); var mol = command.result_object; \
         print('loaded=' + mol.getAtomCount() + ':' + mol.srctype); \
         var writer = cuemol.SDFMolWriter(); writer.attach(mol); writer.setPath({encoded:?}); \
         writer.compress = 'xzip'; writer.base64 = true; writer.convToLink = true; writer.write(); \
         print('linked=' + (mol.src === {encoded:?}) + ':' + mol.srctype); \
         var sceneWriter = cuemol.SceneXMLWriter(); sceneWriter.attach(scene); \
         sceneWriter.setPath({linked:?}); sceneWriter.write(); \
         sceneWriter.embedAll = true; sceneWriter.setPath({embedded:?}); sceneWriter.write(); \
         print('secstr=' + cuemol.MmcifMolReader().loadsecstr);",
            gzip = gzip.to_string_lossy(),
            encoded = encoded.to_string_lossy(),
            linked = linked.to_string_lossy(),
            embedded = embedded.to_string_lossy(),
        ))
        .unwrap();
    assert_eq!(log, "loaded=1:pdb\nlinked=true:sdf\nsecstr=true\n");
    let encoded_bytes = fs::read(&encoded).unwrap();
    let xz = cuemol_core::Base64InStream::new(&encoded_bytes).into_bytes();
    assert!(xz.starts_with(b"\xfd7zXZ\0"));
    let sdf = String::from_utf8(cuemol_core::decode_xz_bytes(&xz).unwrap()).unwrap();
    assert_eq!(cuemol_molstr::read_sdf_str(&sdf).unwrap().atom_count(), 1);
    let xml = fs::read_to_string(&linked).unwrap();
    assert!(xml.contains("<ropts") && xml.contains("compress=\"xzip\""));
    assert!(!xml.contains("START datachunk:"));
    let mut reloaded = DesktopAppState::new();
    reloaded.open_path(&linked).unwrap();
    assert_eq!(reloaded.active_object_summaries()[0].atom_count, 1);
    fs::remove_file(&encoded).unwrap();
    let mut reloaded = DesktopAppState::new();
    reloaded.open_path(&embedded).unwrap();
    let molecule = &reloaded
        .scene_manager()
        .scene(reloaded.active_scene().unwrap())
        .unwrap()
        .objects()[0];
    assert_eq!(molecule.mol().atoms()[0].position.x, 7.0);
    assert!(!fs::read_to_string(embedded).unwrap().contains("<ropts"));
}

#[test]
fn scene_reader_preserves_views_and_load_command_applies_camera_only_when_requested() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("camera.qsc");
    let (mut app, _) = export_scene();
    let mut saved = Camera::new();
    saved.set_name("__current");
    saved.set_center(Vector4::new3(9., 0., 0.));
    let source = app.active_scene().unwrap();
    app.scene_manager_mut()
        .scene_mut(source)
        .unwrap()
        .set_camera(saved);
    app.save_active_scene_qsc(&path).unwrap();
    let target = app.new_scene("target", true).result_scene;
    let first = app.active_view_summary().unwrap().uid;
    let second = app.command_manager.new_view_command(target).unwrap();
    for (uid, x) in [(first, 1.), (second, 2.)] {
        let mut camera = Camera::new();
        camera.set_center(Vector4::new3(x, 0., 0.));
        app.command_manager
            .view_mut(uid)
            .unwrap()
            .set_camera(&camera);
    }
    let empty = app.new_scene("no-views", false).result_scene;
    let (log, _) = app.run_script_js(&format!(
        "var target = cuemol.getScene({target}); var a = cuemol.getView({first}); var b = cuemol.getView({second}); \
         var reader = cuemol.SceneXMLReader(); reader.attach(target); reader.setPath({path:?}); reader.read(); \
         print('reader=' + a.center.x + ':' + b.center.x); \
         reader.detach(); reader.attach(cuemol.getScene({empty})); reader.read(); \
         var command = cuemol.LoadSceneCommand(); command.target_scene = target; command.file_path = {path:?}; \
         command.set_camera = false; command.run(); print('keep=' + a.center.x + ':' + b.center.x); \
         command.set_camera = true; command.run(); print('apply=' + a.center.x + ':' + b.center.x);",
        target=target.get(), first=first.get(), second=second.get(), empty=empty.get(), path=path.to_string_lossy(),
    )).unwrap();
    assert_eq!(log, "reader=1:2\nkeep=1:2\napply=9:9\n");
    assert!(!app
        .command_manager
        .views()
        .iter()
        .any(|view| view.scene_uid() == empty));
}

#[test]
fn scene_writer_embeds_external_auxiliary_data_and_encodes_qdf_payloads() {
    let dir = tempfile::tempdir().unwrap();
    let map_path = dir.path().join("map.qdf");
    let ply_path = dir.path().join("surface.ply");
    let output = dir.path().join("encoded.qsc");
    let map = DensityMap::from_grid_map(GridMap {
        counts: [2, 2, 2],
        origin: Vector4::ZERO,
        delta: [
            Vector4::new3(1., 0., 0.),
            Vector4::new3(0., 1., 0.),
            Vector4::new3(0., 0., 1.),
        ],
        values: (0..8).map(|value| value as f64).collect(),
    });
    fs::write(
        &map_path,
        cuemol_xtal::QdfDenMapWriter::new().write_bytes(&map),
    )
    .unwrap();
    fs::write(&ply_path, "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n0 1 0\n3 0 1 2\n").unwrap();
    let mut app = DesktopAppState::new();
    let scene = app.new_scene("auxiliary", true).result_scene;
    let (log, _) = app.run_script_js(&format!(
        "var command = cuemol.LoadObjectCommand(); command.target_scene = scene; command.file_format = 'qdfmap'; \
         command.file_path = {map:?}; command.object_name = 'map'; command.run(); \
         print('map=' + (command.result_object.uid === scene.getObjectByName('map').uid)); \
         command.file_format = 'ply'; command.file_path = {ply:?}; command.object_name = 'surface'; command.run(); \
         print('surface=' + (command.result_object.uid === scene.getObjectByName('surface').uid) + ':' + command.result_object.srctype); \
         var writer = cuemol.SceneXMLWriter(); writer.attach(scene); writer.setPath({output:?}); \
         writer.embedAll = true; writer.version = 'QDF2'; writer.compress = 'xzip'; writer.base64 = true; writer.write();",
        map=map_path.to_string_lossy(), ply=ply_path.to_string_lossy(), output=output.to_string_lossy(),
    )).unwrap();
    assert_eq!(log, "map=true\nsurface=true:ply\n");
    assert_eq!(app.density_map_summaries(scene).len(), 1);
    assert_eq!(app.surface_summaries(scene).len(), 1);
    let bytes = fs::read(&output).unwrap();
    assert!(bytes.windows(6).any(|slice| slice == b"QDF213"));
    fs::remove_file(map_path).unwrap();
    fs::remove_file(ply_path).unwrap();
    let mut reloaded = DesktopAppState::new();
    reloaded.open_path(output).unwrap();
    let scene = reloaded.active_scene().unwrap();
    assert_eq!(reloaded.density_map_summaries(scene).len(), 1);
    assert_eq!(reloaded.surface_summaries(scene).len(), 1);
    let loaded = reloaded.density_maps.values().next().unwrap();
    assert!((loaded.map.at_float(1, 1, 1) - map.at_float(1, 1, 1)).abs() < 1e-5);
}

#[test]
fn linked_molecules_restore_model_options_and_namd_topology() {
    let dir = tempfile::tempdir().unwrap();
    let pdb = dir.path().join("models.pdb");
    let psf = dir.path().join("topology.psf");
    let coor = dir.path().join("coordinates.coor");
    let qsc = dir.path().join("linked.qsc");
    fs::write(&pdb, "MODEL        1\nATOM      1  CA  GLY A   1       1.000   0.000   0.000  1.00 10.00           C  \nENDMDL\nMODEL        2\nATOM      1  CA  GLY A   1       2.000   0.000   0.000  1.00 10.00           C  \nENDMDL\nEND\n").unwrap();
    fs::write(&psf, "PSF\n\n       1 !NTITLE\n REMARKS topology\n\n       1 !NATOM\n       1 A        1        ALA      CA     CT1     0.100000       12.0110           0\n\n       0 !NBOND: bonds\n").unwrap();
    let mut coordinates = 1_i32.to_le_bytes().to_vec();
    for value in [4_f64, 5., 6.] {
        coordinates.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&coor, coordinates).unwrap();
    let mut app = DesktopAppState::new();
    app.new_scene("linked", true);
    let (log, _) = app.run_script_js(&format!(
        "var reader = cuemol.PDBFileReader(); reader.loadmodel = true; reader.setPath({pdb:?}); \
         var molecule = reader.createDefaultObj(); reader.attach(molecule); reader.read(); \
         molecule.name = 'models'; scene.addObject(molecule); print('models=' + molecule.getAtomCount()); \
         reader = cuemol.NAMDCoorReader(); reader.setPath({coor:?}); reader.setSubPath('topo', {psf:?}); \
         molecule = reader.createDefaultObj(); reader.attach(molecule); reader.read(); \
         molecule.name = 'coordinates'; scene.addObject(molecule); \
         var writer = cuemol.SceneXMLWriter(); writer.attach(scene); writer.setPath({qsc:?}); writer.write();",
        pdb=pdb.to_string_lossy(), coor=coor.to_string_lossy(), psf=psf.to_string_lossy(), qsc=qsc.to_string_lossy(),
    )).unwrap();
    assert_eq!(log, "models=2\n");
    let xml = fs::read_to_string(&qsc).unwrap();
    assert!(!xml.contains("START datachunk:"));
    let mut reloaded = DesktopAppState::new();
    reloaded.open_path(qsc).unwrap();
    let scene = reloaded
        .scene_manager()
        .scene(reloaded.active_scene().unwrap())
        .unwrap();
    assert_eq!(
        scene.object_by_name("models").unwrap().mol().atom_count(),
        2
    );
    assert_eq!(
        scene.object_by_name("coordinates").unwrap().mol().atoms()[0].position,
        Vector4::new3(4., 5., 6.)
    );
}

#[test]
fn compressed_script_trajectory_readers_apply_filters_before_indexing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("frames.dcd.b64");
    let mut block = cuemol_mdtools::TrajBlock::new(1);
    for x in [1_f32, 2., 3.] {
        block.append_frame(vec![x, 0., 0.], None).unwrap();
    }
    let bytes = GuiTrajectoryFrames::Owned(std::sync::Arc::new(block))
        .dcd_bytes(1)
        .unwrap();
    let mut stream = cuemol_qsys::InOutHandler::default();
    stream.set_path(path.to_string_lossy());
    stream.set_compress(cuemol_qsys::InOutHandler::COMP_GZIP);
    stream.set_base64(true);
    stream.write_bytes(&bytes).unwrap();
    let mut app = DesktopAppState::new();
    app.ensure_active_scene();
    let (log, _) = app.run_script_js(&format!(
        "var reader = cuemol.DCDTrajReader(); reader.path = {path:?}; reader.compress = 'gzip'; reader.base64 = true; \
         reader.nevery = 2; var block = reader.createDefaultObj(); reader.attach(block); reader.read(); \
         print('frames=' + block.nframe); reader.compress = 'bzip2'; \
         try {{ reader.read(); print('unsupported=false'); }} catch (e) {{ print('unsupported=true'); }} \
         print('kept=' + block.nframe);", path=path.to_string_lossy(),
    )).unwrap();
    assert_eq!(log, "frames=2\nunsupported=true\nkept=2\n");
}
