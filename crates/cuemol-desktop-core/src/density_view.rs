use super::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct DensityViewSync {
    pub geometry_changed: bool,
    pub pending: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct DensityViewUpdate {
    center: Vector4,
    half: f64,
    dragging: bool,
    changed_at: f64,
    pending: bool,
}

impl DesktopAppState {
    /// Synchronize map regions after navigation; `None` applies a settled view immediately.
    pub fn sync_density_view(
        &mut self,
        camera: &Camera,
        aspect: f64,
        dragging: bool,
        now: Option<f64>,
    ) -> DensityViewSync {
        let Some(scene_uid) = self.active_scene else {
            return DensityViewSync::default();
        };
        let center = camera.center();
        let half = camera.zoom() * aspect.max(1.) * 0.5;
        if ![center.x, center.y, center.z, half]
            .into_iter()
            .all(f64::is_finite)
        {
            return DensityViewSync::default();
        }
        let time = now.unwrap_or(0.);
        let previous = self.density_view_updates.get(&scene_uid).copied();
        let center_changed = previous.is_none_or(|p| (p.center - center).length3d() > 1e-8);
        let changed = center_changed
            || previous.is_none_or(|p| (p.half - half).abs() > 1e-8 || p.dragging != dragging);
        let mut update = previous.unwrap_or(DensityViewUpdate {
            center,
            half,
            dragging,
            changed_at: time,
            pending: true,
        });
        if changed {
            update = DensityViewUpdate {
                center,
                half,
                dragging,
                changed_at: time,
                pending: true,
            };
        }
        let ready = now.is_none() || time - update.changed_at >= 0.15;
        let mut result = DensityViewSync::default();
        let mut centers = Vec::new();
        let mut boxes = Vec::new();
        for object in self
            .density_maps
            .values()
            .filter(|m| m.scene_uid == scene_uid && m.visible)
        {
            for config in object
                .renderers
                .iter()
                .filter(|r| r.visible && r.kind != GuiDensityRendererKind::UnitCell)
            {
                let drag_update = config.attributes.get("dragupdate").map_or(
                    matches!(config.renderer_type.as_str(), "gpu_mapmesh" | "gpu_mapvol"),
                    |v| qsc_bool(v),
                );
                let auto_update = config
                    .attributes
                    .get("autoupdate")
                    .is_none_or(|v| qsc_bool(v));
                let full = density_level_state(&object.map, config).is_full_region(&object.map);
                if (center_changed || previous.is_some_and(|p| p.dragging && !dragging))
                    && (if dragging {
                        drag_update
                    } else {
                        auto_update || drag_update
                    })
                    && (config.center - center).length3d() > 1e-8
                {
                    centers.push((object.uid, config.uid, center));
                    result.geometry_changed |= !full;
                }
                if !full
                    || config
                        .attributes
                        .get("zoom_refine")
                        .is_some_and(|v| !qsc_bool(v))
                {
                    continue;
                }
                if dragging && !drag_update {
                    continue;
                }
                if !ready {
                    result.pending = true;
                    continue;
                }
                if !update.pending && self.density_view_boxes.contains_key(&config.uid) {
                    continue;
                }
                let mut state = density_level_state(&object.map, config);
                state.set_world_transform(object.xform);
                let points = self.density_boundary_points(object, config);
                if let Some((old_center, old_half)) = self.density_view_boxes.get(&config.uid) {
                    state.set_view_box(*old_center, *old_half);
                    let Some(old) = state.full_region(&object.map, points.as_deref()) else {
                        continue;
                    };
                    state.set_view_box(center, half);
                    if !state.view_region_requires_rebuild(&object.map, points.as_deref(), old) {
                        continue;
                    }
                }
                boxes.push(config.uid);
                result.geometry_changed = true;
            }
        }
        for (object, renderer, center) in centers {
            if let Some(map) = self.density_maps.get_mut(&object) {
                if let Some(renderer) = map.renderers.iter_mut().find(|r| r.uid == renderer) {
                    renderer.center = center;
                    renderer
                        .attributes
                        .insert("center".into(), center.to_string());
                    map.modified = true;
                }
            }
        }
        for renderer in boxes {
            self.density_view_boxes.insert(renderer, (center, half));
        }
        if ready {
            update.pending = false;
        }
        self.density_view_updates.insert(scene_uid, update);
        let renderers = self
            .density_maps
            .values()
            .flat_map(|m| m.renderers.iter().map(|r| r.uid))
            .collect::<BTreeSet<_>>();
        self.density_view_boxes
            .retain(|uid, _| renderers.contains(uid));
        result
    }

    pub(super) fn configure_density_view(
        &self,
        state: &mut MapRenderer,
        object: &GuiDensityMapObject,
        config: &GuiDensityRenderer,
    ) {
        state.set_world_transform(object.xform);
        if let Some((center, half)) = self.density_view_boxes.get(&config.uid) {
            state.set_view_box(*center, *half);
        }
    }

    pub(super) fn density_boundary_points(
        &self,
        map: &GuiDensityMapObject,
        config: &GuiDensityRenderer,
    ) -> Option<Vec<Vector4>> {
        if config.bndry_molname.is_empty() {
            return None;
        }
        let object = self
            .scene_manager()
            .scene(map.scene_uid)?
            .objects()
            .iter()
            .find(|o| o.name() == config.bndry_molname)?;
        let transform = map.xform.invert().ok()?.mul_mat(object.xform_matrix());
        let expression = config
            .attributes
            .get("bndry_sel")
            .cloned()
            .unwrap_or_else(|| config.bndry_sel.expression());
        let selector = self.parse_object_selector(object.uid(), &expression).ok()?;
        Some(
            object
                .mol()
                .select_indices(&selector)
                .into_iter()
                .map(|i| {
                    let p = object.mol().atoms()[i].position;
                    transform.transform_point(Vector4::new4(p.x, p.y, p.z, 1.))
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn radial_map(n: usize, origin: Vector4) -> DensityMap {
        let mut map = DensityMap::new();
        let center = (n - 1) as f64 * 0.5;
        let bytes = (0..n * n * n)
            .map(|i| {
                let p = Vector4::new3((i % n) as f64, ((i / n) % n) as f64, (i / n / n) as f64);
                let d = p - Vector4::new3(center, center, center);
                (200. - d.length3d() * 10.).clamp(0., 255.) as u8
            })
            .collect::<Vec<_>>();
        map.set_map_byte_array(&bytes, [n; 3], 0., 256., 20., 10.);
        map.set_map_params([0; 3], [n; 3]);
        map.set_xtal_params(n as f64, n as f64, n as f64, 90., 90., 90.);
        map.set_origin(origin);
        map.set_detected_map_type(2);
        map
    }

    #[test]
    fn boundary_points_rebind_scene_distances_before_map_coordinate_conversion() {
        let mut app = DesktopAppState::new();
        let scene_id = app.new_scene("boundary", true).result_scene;
        let molecule = cuemol_molstr::read_pdb_str("HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00  0.00           C\nHETATM    2  O1  LIG A   1       1.400   0.000   0.000  1.00  0.00           O\nEND\n").unwrap();
        let scene = app.scene_manager_mut().scene_mut(scene_id).unwrap();
        let probe = scene.add_molecule("Probe", molecule.clone());
        let target = scene.add_molecule("Target", molecule);
        let map_id = app.add_density_map(scene_id, "map", radial_map(9, Vector4::ZERO));
        app.add_density_renderer(map_id, "isosurf").unwrap();
        let map = app.density_maps.get_mut(&map_id).unwrap();
        map.xform.set_trans_part(Vector4::new3(5., 0., 0.));
        map.renderers[0].bndry_molname = "Probe".into();
        map.renderers[0].bndry_sel = parse_selector("name C1 around 2 [Target]").unwrap();
        let points = |app: &DesktopAppState| {
            let map = &app.density_maps[&map_id];
            app.density_boundary_points(map, &map.renderers[0])
                .unwrap()
                .into_iter()
                .map(|point| [point.x, point.y, point.z])
                .collect::<Vec<_>>()
        };
        assert_eq!(points(&app), [[-5., 0., 0.], [-3.6, 0., 0.]]);
        let mut transform = Matrix4::IDENTITY;
        transform.set_trans_part(Vector4::new3(10., 0., 0.));
        app.scene_manager_mut()
            .object_mut(target)
            .unwrap()
            .set_xform_matrix(transform);
        assert!(points(&app).is_empty());
        app.scene_manager_mut()
            .object_mut(probe)
            .unwrap()
            .set_xform_matrix(transform);
        assert_eq!(points(&app), [[5., 0., 0.], [6.4, 0., 0.]]);
    }

    #[test]
    fn navigation_refines_after_debounce_and_keeps_covered_regions() {
        let mut app = DesktopAppState::new();
        let scene = app.new_scene("map navigation", true).result_scene;
        let map = app.add_density_map(scene, "em", radial_map(129, Vector4::ZERO));
        let renderer = app.add_density_renderer(map, "isosurf").unwrap();
        app.density_maps.get_mut(&map).unwrap().renderers[0]
            .attributes
            .insert("lod_budget".into(), "1".into());
        let mut camera = Camera::new();
        camera.set_center(Vector4::new3(64., 64., 64.));
        camera.set_zoom(200.);
        assert!(app.sync_density_view(&camera, 1., false, Some(0.)).pending);
        assert!(
            app.sync_density_view(&camera, 1., false, Some(0.16))
                .geometry_changed
        );
        let mut state = density_level_state(
            &app.density_maps[&map].map,
            &app.density_maps[&map].renderers[0],
        );
        state.set_view_box(camera.center(), 100.);
        assert_eq!(
            state
                .full_region(&app.density_maps[&map].map, None)
                .unwrap()
                .step,
            2
        );
        camera.set_zoom(12.);
        assert!(app.sync_density_view(&camera, 1., false, Some(1.)).pending);
        assert!(
            !app.sync_density_view(&camera, 1., false, Some(1.1))
                .geometry_changed
        );
        assert!(
            app.sync_density_view(&camera, 1., false, Some(1.16))
                .geometry_changed
        );
        assert_eq!(app.density_view_boxes[&renderer].1, 6.);
        camera.set_center(Vector4::new3(65., 64., 64.));
        app.sync_density_view(&camera, 1., false, Some(2.));
        assert!(
            !app.sync_density_view(&camera, 1., false, Some(2.16))
                .geometry_changed
        );
        camera.set_center(Vector4::new3(80., 64., 64.));
        app.sync_density_view(&camera, 1., true, Some(3.));
        assert!(
            !app.sync_density_view(&camera, 1., true, Some(3.16))
                .geometry_changed
        );
        app.sync_density_view(&camera, 1., false, Some(4.));
        assert!(
            app.sync_density_view(&camera, 1., false, Some(4.16))
                .geometry_changed
        );
        camera.set_center(Vector4::new3(1000., 1000., 1000.));
        assert!(
            !app.sync_density_view(&camera, 1., false, None)
                .geometry_changed
        );
    }

    #[test]
    #[cfg(feature = "scripting")]
    fn map_region_scripts_change_geometry_and_survive_scene_storage() {
        let mut app = DesktopAppState::new();
        let scene = app.new_scene("map regions", true).result_scene;
        app.add_density_map(scene, "em", radial_map(33, Vector4::new3(80., 0., 0.)));
        let (log,_) = app.run_script_js("var m=cuemol.getObject('em'); var r=m.createRenderer('isosurf'); r.name='surface'; r.level=70; print(r.region_mode+':'+r.region_mode_resolved+':'+r.lod+':'+r.lod_budget+':'+r.zoom_refine); var q=cuemol.MapMeshRenderer(); q.zoom_refine=false; q.lod_budget=-4; q.region_mode='full'; var bad=false; try {q.lod='step3'} catch(e) {bad=true}; print(q.zoom_refine+':'+q.lod_budget+':'+bad+':'+q.region_mode_resolved);").unwrap();
        assert_eq!(log, "auto:full:auto:16:true\nfalse:1:true:full\n");
        let fine = app
            .active_render_snapshot("")
            .unwrap()
            .mesh_triangle_count();
        assert!(fine > 100);
        app.run_script_js("cuemol.getObject('em').getRendererByType('isosurf').region_mode='box';")
            .unwrap();
        assert_eq!(
            app.active_render_snapshot("")
                .unwrap()
                .mesh_triangle_count(),
            0
        );
        app.run_script_js("var r=cuemol.getObject('em').getRendererByType('isosurf'); r.region_mode='full'; r.lod='step4'; r.zoom_refine=false;").unwrap();
        let coarse = app
            .active_render_snapshot("")
            .unwrap()
            .mesh_triangle_count();
        assert!(coarse > 0 && coarse < fine / 2);
        let bytes = app.active_scene_bytes().unwrap();
        let mut restored = DesktopAppState::new();
        restored.open_bytes("regions.qsc", &bytes).unwrap();
        assert_eq!(
            restored
                .active_render_snapshot("")
                .unwrap()
                .mesh_triangle_count(),
            coarse
        );
        assert_eq!(restored.run_script_js("var r=cuemol.getObject('em').getRendererByType('isosurf'); print(r.region_mode_resolved+':'+r.lod+':'+r.zoom_refine); cuemol.resetProp(r,'lod'); print(r.lod);").unwrap().0,"full:step4:false\nauto\n");
        assert_eq!(
            restored
                .active_render_snapshot("")
                .unwrap()
                .mesh_triangle_count(),
            fine
        );
    }
}
