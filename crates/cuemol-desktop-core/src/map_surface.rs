use super::*;
use cuemol_core::BSPTree;

impl DesktopAppState {
    pub(super) fn density_surface_vertices(
        &self,
        object: &GuiDensityMapObject,
        renderer: &GuiDensityRenderer,
        mesh: &cuemol_surface::SurfaceMesh,
        styles: &StyleManager,
    ) -> Vec<cuemol_render::MeshVertex> {
        let fallback = renderer
            .color
            .rgba8()
            .map(rgba8_render_color)
            .unwrap_or([0.85, 0.85, 0.85, 1.0]);
        let attribute = |key: &str| {
            renderer
                .attributes
                .get(key)
                .cloned()
                .or_else(|| {
                    let names = renderer.attributes.get("style")?;
                    styles.resolve_style_attr(object.scene_uid.get(), names, key)
                })
                .unwrap_or_default()
        };
        let palette = (renderer.color_mode == MapColorMode::Molecule)
            .then(|| {
                let scene = self.scene_manager().scene(object.scene_uid)?;
                let target_name = attribute("target");
                let target = scene.objects().iter().find(|o| o.name() == target_name)?;
                let selection = attribute("sel");
                let selector = parse_selector_with_styles(
                    if selection.is_empty() {
                        "*"
                    } else {
                        &selection
                    },
                    styles,
                    object.scene_uid.get(),
                )
                .ok()?;
                let node = density_renderer_qsc_node(renderer);
                let mut options = MoleculeRenderOptions::default();
                cuemol_molvis::apply_molecular_node_coloring(
                    &mut options,
                    target,
                    Some(&node),
                    styles,
                    object.scene_uid.get(),
                    &qsc_rgba8_string(renderer.color.rgba8().unwrap_or(cuemol_gfx::Rgba8 {
                        r: 217,
                        g: 217,
                        b: 217,
                        a: 255,
                    })),
                );
                let colors = resolve_atom_colors(target.mol(), &options);
                let mut index = BSPTree::new();
                for i in target.mol().select_indices(&selector) {
                    let mut position = target.mol().atoms()[i].position;
                    target.xform_matrix().xform3d(&mut position);
                    let color = colors
                        .get(i)
                        .and_then(|c| c.rgba8())
                        .map(rgba8_render_color)
                        .unwrap_or(fallback);
                    index.push(position, color);
                }
                index.build();
                Some(index)
            })
            .flatten();
        let color_map = (renderer.color_mode == MapColorMode::MultiGradient)
            .then(|| {
                self.density_maps
                    .values()
                    .find(|m| m.scene_uid == object.scene_uid && m.name == renderer.color_mapname)
                    .and_then(|m| m.xform.invert().ok().map(|inverse| (m, inverse)))
            })
            .flatten();
        mesh.vertices()
            .iter()
            .map(|vertex| {
                let mut world = vertex.position;
                object.xform.xform3d(&mut world);
                let color = if let Some(index) = &palette {
                    index.nearest(world).copied().unwrap_or(fallback)
                } else if let Some((map, inverse)) = color_map {
                    inverse.xform3d(&mut world);
                    rgba8_render_color(renderer.multi_grad.color(map.map.value_at(world)))
                } else {
                    fallback
                };
                cuemol_render::MeshVertex {
                    position: vertex.position,
                    normal: vertex.normal,
                    color,
                }
            })
            .collect()
    }

    pub(super) fn set_density_coloring(
        &mut self,
        object_uid: Uid,
        renderer_uid: Uid,
        coloring_type: Option<String>,
        attrs: BTreeMap<String, String>,
        paint_entries: Vec<(String, String)>,
        default_color: Option<String>,
    ) -> Result<()> {
        let map = self
            .density_maps
            .get(&object_uid)
            .ok_or_else(|| CueMolError::not_found("density map"))?;
        let styles = self
            .scene_manager()
            .scene(map.scene_uid)
            .map(scene_style_manager)
            .unwrap_or_else(cuemol_molvis::default_style_manager);
        for (selection, color) in &paint_entries {
            parse_selector_with_styles(selection, &styles, map.scene_uid.get())?;
            cuemol_gfx::parse_color(color)?;
        }
        let default_color = default_color
            .as_deref()
            .map(|value| {
                resolve_qsc_color(value, &styles).ok_or_else(|| {
                    CueMolError::invalid_argument(format!("invalid surface color: {value}"))
                })
            })
            .transpose()?;
        let map = self.density_maps.get_mut(&object_uid).unwrap();
        let renderer = map
            .renderers
            .iter_mut()
            .find(|r| r.uid == renderer_uid)
            .ok_or_else(|| CueMolError::not_found("density renderer"))?;
        if renderer.kind != GuiDensityRendererKind::IsoSurface {
            return Err(CueMolError::invalid_argument(
                "renderer does not support molecular coloring",
            ));
        }
        renderer
            .children
            .retain(|child| child.tag_name() != "coloring");
        if let Some(kind) = coloring_type {
            let mut node = LDom2Node::new();
            node.set_tag_name("coloring");
            node.set_type_name(&kind);
            for (key, value) in attrs {
                if kind == "ScriptColoring" && key == "script" {
                    node.append_child("script").set_contents(value);
                } else {
                    node.set_str_attr(key, value);
                }
            }
            for (selection, color) in paint_entries {
                let entry = node.append_child("paint");
                entry.set_str_attr("sel", selection);
                entry.set_str_attr("color", color);
            }
            renderer.children.push(node);
        }
        renderer.color_mode = MapColorMode::Molecule;
        renderer
            .attributes
            .insert("colormode".into(), "molecule".into());
        if let Some(color) = default_color {
            renderer
                .attributes
                .insert("color".into(), qsc_rgba8_string(color));
            renderer.color = render_color_spec(rgba8_render_color(color));
        }
        map.modified = true;
        Ok(())
    }
}

#[cfg(all(test, feature = "scripting"))]
mod tests {
    use super::*;

    #[test]
    fn map_caps_molecular_colors_and_children_survive_live_edits_and_scene_storage() {
        let mut app = DesktopAppState::new();
        let sid = app.ensure_active_scene();
        let molecule = cuemol_molstr::read_pdb_str(
            "ATOM      1  C   GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \n\
             ATOM      2  O   GLY A   1       1.000   0.000   0.000  1.00 10.00           O  \nEND\n",
        ).unwrap();
        let mol = app
            .scene_manager_mut()
            .scene_mut(sid)
            .unwrap()
            .add_molecule("target", molecule);
        let mut map = DensityMap::new();
        map.set_map_byte_array(&[192; 8], [2; 3], 0., 256., 128., 64.);
        map.set_map_params([0; 3], [2; 3]);
        map.set_xtal_params(2., 2., 2., 90., 90., 90.);
        let mid = app.add_density_map(sid, "map", map);
        let rid = app.add_density_renderer(mid, "isosurf").unwrap();
        let log = app
            .run_script_js(&format!(
                r#"
            var r=cuemol.getRenderer({});
            print(r.cap_mode+':'+r.target+':'+cuemol.getClassName(r.coloring)+':'+r.color.r());
            r.region_mode='full'; r.level=128; r.target='target'; r.colormode='molecule';
            var p=cuemol.PaintColoring();
            p.append(cuemol.makeSel('aid 1'),cuemol.makeColor('#ff0000'));
            p.append(cuemol.makeSel('aid 2'),cuemol.makeColor('#0000ff'));
            r.coloring=p;
        "#,
                rid.get()
            ))
            .unwrap()
            .0;
        assert_eq!(log, "auto::SolidColoring:217\n");
        let colors = |app: &DesktopAppState| {
            let snapshot = app.active_render_snapshot("").unwrap();
            assert_eq!(snapshot.mesh_triangle_count(), 12);
            snapshot
                .meshes
                .iter()
                .flat_map(|m| &m.vertices)
                .map(|v| v.color.map(|c| (c * 255.).round() as u8))
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(
            colors(&app),
            BTreeSet::from([[255, 0, 0, 255], [0, 0, 255, 255]])
        );
        // A translated target changes the nearest atom in world coordinates.
        let mut transform = Matrix4::IDENTITY;
        transform.values[0][3] = 10.;
        app.scene_manager_mut()
            .object_mut(mol)
            .unwrap()
            .set_xform_matrix(transform);
        assert_eq!(colors(&app), BTreeSet::from([[255, 0, 0, 255]]));
        app.scene_manager_mut()
            .object_mut(mol)
            .unwrap()
            .set_xform_matrix(Matrix4::IDENTITY);
        app.run_script_js(&format!(
            "cuemol.getRenderer({}).sel=cuemol.makeSel('aid 2');",
            rid.get()
        ))
        .unwrap();
        assert_eq!(colors(&app), BTreeSet::from([[0, 0, 255, 255]]));
        let bytes = app.active_scene_bytes().unwrap();
        let mut loaded = DesktopAppState::new();
        loaded.open_bytes("caps.qsc", &bytes).unwrap();
        assert_eq!(colors(&loaded), BTreeSet::from([[0, 0, 255, 255]]));
        assert_eq!(loaded.run_script_js("var r=cuemol.getObject('map').getRendererByType('isosurf'); print(r.coloring.size+':'+r.sel); r.coloring.changeAt(1,cuemol.makeSel('aid 2'),cuemol.makeColor('#00ff00'));").unwrap().0,"2:aid 2\n");
        assert_eq!(colors(&loaded), BTreeSet::from([[0, 255, 0, 255]]));
        loaded
            .run_script_js(
                "var r=cuemol.getObject('map').getRendererByType('isosurf'); r.cap_mode='off';",
            )
            .unwrap();
        assert_eq!(
            loaded
                .active_render_snapshot("")
                .unwrap()
                .mesh_triangle_count(),
            0
        );
        loaded.run_script_js("var r=cuemol.getObject('map').getRendererByType('isosurf'); cuemol.resetProp(r,'cap_mode'); r.region_mode='box'; r.use_pbc=false; print(cuemol.getClassName(r.generateSurfObj()));").map(|r|assert_eq!(r.0,"MolSurfObj\n")).unwrap();
        assert_eq!(
            loaded
                .active_render_snapshot("")
                .unwrap()
                .mesh_triangle_count(),
            0
        );
        loaded.run_script_js("var m=cuemol.getObject('map'); var r=cuemol.MapSurfRenderer(); r.name='detached'; r.region_mode='full'; r.siglevel=2; r.target='target'; r.colormode='molecule'; r.sel=cuemol.makeSel('aid 1'); var p=cuemol.PaintColoring(); p.append(cuemol.makeSel('*'),cuemol.makeColor('#123456')); r.coloring=p; m.attachRenderer(r); var invalid=false; try{r.cap_mode='bad'}catch(e){invalid=true}; print(invalid+':'+r.cap_mode);").map(|r|assert_eq!(r.0,"true:auto\n")).unwrap();
        assert_eq!(colors(&loaded), BTreeSet::from([[18, 52, 86, 255]]));
        let map_uid = loaded
            .density_maps
            .values()
            .find(|m| m.name == "map")
            .unwrap()
            .uid;
        let mut transform = Matrix4::IDENTITY;
        transform.values[0][0] = -2.;
        transform.values[1][1] = 3.;
        transform.values[0][3] = 10.;
        loaded.density_maps.get_mut(&map_uid).unwrap().xform = transform;
        loaded.run_script_js("var m=cuemol.getObject('map'); var g=m.getRendererByName('detached').generateSurfObj(); g.name='generated'; cuemol.getScene().addObject(g);").unwrap();
        let scene = loaded
            .scene_manager()
            .scene(loaded.active_scene().unwrap())
            .unwrap();
        let generated = loaded_surface_object(
            scene
                .loaded_surfaces()
                .iter()
                .find(|s| s.name == "generated")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(generated.mesh().triangle_count(), 12);
        let center = Vector4::new3(9., 1.5, 0.5);
        for v in generated.mesh().vertices() {
            assert!(v.position.x >= 8. - 1e-6 && v.position.x <= 10. + 1e-6);
            assert!(v.position.y >= -1e-6 && v.position.y <= 3. + 1e-6);
            assert!(v.normal.dot3d(v.position - center) > 0.49);
        }
        for t in generated.mesh().triangles() {
            let [a, b, c] = t.map(|i| &generated.mesh().vertices()[i as usize]);
            assert!(
                (b.position - a.position)
                    .cross(c.position - a.position)
                    .dot3d(a.normal)
                    > 0.
            );
        }
    }
}
