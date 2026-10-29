use crate::DesktopAppState;
use cuemol_core::clock::Instant;
use cuemol_core::{time_to_milliseconds, CueMolError, Result, Uid};
use cuemol_qsys::Camera;
use std::time::Duration;

#[derive(Clone, Debug)]
pub(crate) struct Playback {
    view: Uid,
    camera: Camera,
    elapsed: i64,
    running: bool,
    last_tick: Instant,
}

impl DesktopAppState {
    pub fn scene_animation_changes_geometry(&self, scene: Uid) -> Result<bool> {
        Ok(self.scene_animation(scene)?.is_some_and(|manager| {
            manager.motions().iter().any(|motion| {
                !matches!(
                    motion,
                    cuemol_anim::AnimMotion::Noop(_)
                        | cuemol_anim::AnimMotion::Camera(_)
                        | cuemol_anim::AnimMotion::Spin(_)
                )
            })
        }))
    }

    pub fn play_scene_animation(&mut self, scene: Uid, view: Uid) -> Result<()> {
        if self.is_scene_animation_playing(scene) {
            return Ok(());
        }
        let mut manager = self.scene_animation(scene)?.unwrap_or_default();
        if manager.length() <= 0 {
            return Err(CueMolError::invalid_argument("animation has no duration"));
        }
        let view = self
            .command_manager()
            .view(view)
            .filter(|view| view.scene_uid() == scene)
            .cloned()
            .ok_or_else(|| CueMolError::not_found("animation view"))?;
        let elapsed = manager.elapsed().clamp(0, manager.length());
        let elapsed = if elapsed == manager.length() {
            0
        } else {
            elapsed
        };
        let camera = manager
            .initial_camera()
            .cloned()
            .unwrap_or_else(|| view.camera());
        manager.start(&view);
        self.set_scene_animation(scene, Some(&manager))?;
        self.command_manager_mut()
            .view_mut(view.uid())
            .unwrap()
            .set_camera(&camera);
        self.update_scene_animation(scene, view.uid(), Some(elapsed))?;
        let mut manager = self.scene_animation(scene)?.unwrap();
        manager.run();
        self.set_scene_animation(scene, Some(&manager))?;
        self.animation_players.insert(
            scene,
            Playback {
                view: view.uid(),
                camera,
                elapsed,
                running: true,
                last_tick: Instant::now(),
            },
        );
        Ok(())
    }

    pub fn pause_scene_animation(&mut self, scene: Uid) -> Result<Option<Camera>> {
        let camera = self.advance_scene_animation(scene)?;
        if let Some(player) = self.animation_players.get_mut(&scene) {
            player.running = false;
        }
        if let Some(mut manager) = self.scene_animation(scene)? {
            manager.pause();
            self.set_scene_animation(scene, Some(&manager))?;
        }
        Ok(camera)
    }

    pub fn is_scene_animation_playing(&self, scene: Uid) -> bool {
        self.animation_players
            .get(&scene)
            .is_some_and(|player| player.running)
    }

    pub fn has_scene_animation_player(&self, scene: Uid) -> bool {
        self.animation_players.contains_key(&scene)
    }

    pub fn scene_animation_elapsed_ms(&self, scene: Uid) -> f64 {
        self.animation_players
            .get(&scene)
            .map(|player| time_to_milliseconds(player.elapsed))
            .or_else(|| {
                self.scene_animation(scene)
                    .ok()
                    .flatten()
                    .map(|manager| time_to_milliseconds(manager.elapsed()))
            })
            .unwrap_or(0.0)
    }

    pub fn advance_scene_animation(&mut self, scene: Uid) -> Result<Option<Camera>> {
        let Some(player) = self
            .animation_players
            .get(&scene)
            .filter(|player| player.running)
        else {
            return Ok(None);
        };
        self.advance_scene_animation_by(scene, player.last_tick.elapsed())
    }

    pub fn advance_scene_animation_by(
        &mut self,
        scene: Uid,
        delta: Duration,
    ) -> Result<Option<Camera>> {
        let Some(mut player) = self.animation_players.remove(&scene) else {
            return Ok(None);
        };
        if !player.running {
            self.animation_players.insert(scene, player);
            return Ok(None);
        }
        let Some(manager) = self.scene_animation(scene)? else {
            return Ok(None);
        };
        let duration = manager.length();
        if duration <= 0 || self.command_manager().view(player.view).is_none() {
            return self
                .update_scene_animation(scene, Uid::new(0), None)
                .map(Some);
        }
        let elapsed = player
            .elapsed
            .saturating_add(delta.as_nanos().min(i64::MAX as u128) as i64);
        let finished = !manager.loop_enabled() && elapsed >= duration;
        let elapsed = if manager.loop_enabled() {
            elapsed % duration
        } else {
            elapsed.min(duration)
        };
        self.command_manager_mut()
            .view_mut(player.view)
            .unwrap()
            .set_camera(&player.camera);
        let mut camera = self.update_scene_animation(scene, player.view, Some(elapsed))?;
        if finished {
            camera = self.update_scene_animation(scene, player.view, None)?;
            player.elapsed = 0;
            player.running = false;
        } else {
            let mut manager = self.scene_animation(scene)?.unwrap();
            manager.run();
            self.set_scene_animation(scene, Some(&manager))?;
            player.elapsed = elapsed;
        }
        player.last_tick = Instant::now();
        self.animation_players.insert(scene, player);
        Ok(Some(camera))
    }

    pub(crate) fn pause_animation_for_seek(&mut self, scene: Uid, elapsed: i64) {
        if let Some(player) = self.animation_players.get_mut(&scene) {
            player.running = false;
            player.elapsed = elapsed;
        }
    }
}

#[cfg(all(test, feature = "scripting"))]
mod tests {
    use super::*;
    use cuemol_core::{time_from_milliseconds, Vector4};

    fn playing_scene() -> (DesktopAppState, Uid, Uid, Uid, Uid) {
        let mut app = DesktopAppState::new();
        let created = app.new_scene("playback", true);
        let scene = created.result_scene;
        let view = created.result_view.unwrap();
        let mol = cuemol_molstr::read_pdb_str(
            "ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n",
        ).unwrap();
        let object = app
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .add_molecule("mol", mol);
        let renderer = app
            .new_renderer(scene, object, "cpk", "atoms")
            .unwrap()
            .result_renderer;
        app.run_script_js(&format!(
            "function time(ms) {{ var t = cuemol.TimeValue(); t.millisec = ms; return t; }} \
             cuemol.getRenderer({renderer}).alpha = 0.8; \
             var manager = scene.getAnimMgr(); manager.length = time(1000); \
             var fade = cuemol.RealPropAnim(); fade.rend = 'mol/atoms'; fade.prop = 'alpha'; \
             fade.startValue = 0; fade.endValue = 1; fade.end = time(1000); manager.append(fade); \
             var spin = cuemol.SimpleSpin(); spin.angle = 180; spin.end = time(1000); manager.append(spin); \
             manager.start(cuemol.getView({view}));",
            renderer=renderer.get(),view=view.get(),
        )).unwrap();
        (app, scene, view, object, renderer)
    }

    #[test]
    fn replacing_a_scene_cancels_its_previous_animation_player() {
        let (mut app, scene, _, _, _) = playing_scene();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("replacement.qsc");
        std::fs::write(
            &path,
            "<scene><camera name=\"__current\" center=\"(4,5,6)\"/></scene>",
        )
        .unwrap();
        app.open_scene_file(path, Some(scene)).unwrap();
        assert!(!app.is_scene_animation_playing(scene));
        assert!(!app.has_scene_animation_player(scene));
        assert!(app.scene_animation(scene).unwrap().is_none());
        let camera = app.active_view_camera().unwrap();
        assert!(app
            .advance_scene_animation_by(scene, Duration::from_millis(100))
            .unwrap()
            .is_none());
        assert_eq!(app.active_view_camera().unwrap(), camera);
    }

    fn alpha(app: &DesktopAppState, object: Uid, renderer: Uid) -> f64 {
        app.scene_manager()
            .object(object)
            .unwrap()
            .renderer(renderer)
            .unwrap()
            .qsc_attr("alpha")
            .unwrap()
            .parse()
            .unwrap()
    }

    #[test]
    fn script_started_playback_advances_and_restores_on_stop() {
        let (mut app, scene, view, object, renderer) = playing_scene();
        assert!(app.is_scene_animation_playing(scene));
        assert_eq!(alpha(&app, object, renderer), 0.0);
        let initial = app.command_manager().view(view).unwrap().rotation();
        app.advance_scene_animation_by(scene, Duration::from_millis(250))
            .unwrap()
            .unwrap();
        assert_eq!(alpha(&app, object, renderer), 0.25);
        assert_ne!(
            app.command_manager().view(view).unwrap().rotation(),
            initial
        );
        app.run_script_js("scene.getAnimMgr().pause();").unwrap();
        assert!(!app.is_scene_animation_playing(scene));
        let paused = app.scene_animation_elapsed_ms(scene);
        assert!(app
            .advance_scene_animation_by(scene, Duration::from_secs(3))
            .unwrap()
            .is_none());
        assert_eq!(app.scene_animation_elapsed_ms(scene), paused);
        app.run_script_js(&format!(
            "scene.getAnimMgr().start(cuemol.getView({}));",
            view.get()
        ))
        .unwrap();
        app.advance_scene_animation_by(scene, Duration::from_millis(100))
            .unwrap();
        assert!((app.scene_animation_elapsed_ms(scene) - paused - 100.0).abs() < 1e-6);
        app.run_script_js("scene.getAnimMgr().stop();").unwrap();
        assert!(!app.is_scene_animation_playing(scene));
        assert_eq!(app.scene_animation_elapsed_ms(scene), 0.0);
        assert_eq!(alpha(&app, object, renderer), 0.8);
    }

    #[test]
    fn loops_and_natural_completion_preserve_pre_animation_properties() {
        let (mut app, scene, view, object, renderer) = playing_scene();
        app.run_script_js("scene.getAnimMgr().loop = true;")
            .unwrap();
        app.advance_scene_animation_by(scene, Duration::from_millis(1500))
            .unwrap();
        assert_eq!(alpha(&app, object, renderer), 0.5);
        assert_eq!(app.scene_animation_elapsed_ms(scene), 500.0);
        app.advance_scene_animation_by(scene, Duration::from_millis(1000))
            .unwrap();
        assert_eq!(alpha(&app, object, renderer), 0.5);
        app.run_script_js("scene.getAnimMgr().loop = false;")
            .unwrap();
        app.advance_scene_animation_by(scene, Duration::from_millis(500))
            .unwrap();
        assert!(!app.is_scene_animation_playing(scene));
        assert_eq!(alpha(&app, object, renderer), 0.8);
        assert_eq!(app.scene_animation_elapsed_ms(scene), 0.0);
        app.apply_scene_animation_time(scene, time_from_milliseconds(300.0), view)
            .unwrap();
        assert_eq!(alpha(&app, object, renderer), 0.3);
        app.stop_scene_animation(scene, view).unwrap();
        assert_eq!(alpha(&app, object, renderer), 0.8);
    }

    #[test]
    fn playback_is_scene_scoped_and_removing_its_view_restores_properties() {
        let (mut app, scene, view, object, renderer) = playing_scene();
        let other = app.new_scene("other", true);
        let mut camera = app.active_view_camera().unwrap();
        camera.set_center(Vector4::new3(9.0, 2.0, 3.0));
        app.set_active_view_camera(&camera).unwrap();
        app.advance_scene_animation_by(scene, Duration::from_millis(250))
            .unwrap();
        assert_eq!(app.active_view_camera().unwrap(), camera);
        app.command_manager_mut()
            .destroy_view_command(view)
            .unwrap();
        app.advance_scene_animation_by(scene, Duration::from_millis(250))
            .unwrap();
        assert!(!app.is_scene_animation_playing(scene));
        assert_eq!(alpha(&app, object, renderer), 0.8);
        assert_eq!(app.command_manager().views().len(), 1);
        assert_eq!(app.active_scene(), Some(other.result_scene));
        app.destroy_scene(scene);
        assert!(app.advance_scene_animation(scene).unwrap().is_none());
    }
}
