use super::*;
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;

fn scene() -> (DesktopAppState, Camera, Value) {
    let mut state = DesktopAppState::new();
    state.new_scene("timeline", true);
    let mut camera = Camera::new();
    for name in ["base", "child", "last"] {
        cuemol_command::call_tool(
            &mut state,
            &mut camera,
            "add_anim",
            &json!({"type":"spin","name":name}),
        )
        .unwrap();
    }
    let data = cuemol_command::animation_timeline(&state).unwrap();
    (state, camera, data)
}
fn point(rect: [f32; 4]) -> egui::Pos2 {
    pos2(rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.)
}
fn pointer(p: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(p),
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        },
    ]
}
fn escape() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }
}
struct Harness {
    ctx: egui::Context,
    frame: usize,
    panel: TimelinePanel,
}
impl Harness {
    fn new(data: &Value) -> Self {
        let mut this = Self {
            ctx: egui::Context::default(),
            frame: 0,
            panel: Default::default(),
        };
        this.step(data, vec![]);
        this.step(data, vec![]);
        this
    }
    fn step(&mut self, data: &Value, events: Vec<egui::Event>) -> Output {
        self.frame += 1;
        let mut result = Output::default();
        let _ = self.ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1000., 400.))),
                time: Some(self.frame as f64 / 60.),
                events,
                focused: true,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    result = self.panel.show(ui, data, true);
                });
            },
        );
        result
    }
}

#[test]
fn strip_drag_commits_once_and_moves_the_chained_span() {
    let (mut state, mut camera, data) = scene();
    let mut h = Harness::new(&data);
    let id = uid(&data["elements"][0]);
    let p = point(h.panel.rects[&format!("Animation strip {id}")]);
    assert!(h.step(&data, pointer(p, true)).commands.is_empty());
    assert!(h
        .step(&data, vec![egui::Event::PointerMoved(p + vec2(30., 0.))])
        .commands
        .is_empty());
    assert!(h.panel.drag.as_ref().is_some_and(|drag| drag.moved));
    assert_eq!(cuemol_command::animation_timeline(&state).unwrap(), data);
    let out = h.step(&data, pointer(p + vec2(30., 0.), false));
    assert_eq!(out.commands.len(), 1);
    let (tool, args) = &out.commands[0];
    assert_eq!(*tool, "set_anim_time");
    assert_eq!(args["startMs"], 300.);
    assert_eq!(args["endMs"], 1300.);
    cuemol_command::call_tool(&mut state, &mut camera, tool, args).unwrap();
    let after = cuemol_command::animation_timeline(&state).unwrap();
    assert_eq!(after["elements"][1]["absStartMs"], 1300.);
    assert!(h.step(&after, vec![]).commands.is_empty());
    assert_eq!(h.panel.selected, Some(id));
}

#[test]
fn strip_drag_cancels_on_escape_scene_change_and_external_timing_edit() {
    let (_, _, data) = scene();
    let id = uid(&data["elements"][0]);
    for change in ["escape", "scene", "time"] {
        let mut h = Harness::new(&data);
        let p = point(h.panel.rects[&format!("Animation strip {id}")]);
        h.step(&data, pointer(p, true));
        h.step(&data, vec![egui::Event::PointerMoved(p + vec2(25., 0.))]);
        let mut updated = data.clone();
        let events = match change {
            "escape" => vec![escape()],
            "scene" => {
                updated["sceneId"] = json!(999);
                vec![]
            }
            _ => {
                updated["elements"][0]["endMs"] = json!(1500);
                vec![]
            }
        };
        assert!(h.step(&updated, events).commands.is_empty());
        assert!(h
            .step(&updated, pointer(p + vec2(25., 0.), false))
            .commands
            .is_empty());
    }
}

#[test]
fn ruler_scrub_previews_locally_and_commits_at_release() {
    let (_, _, data) = scene();
    let mut h = Harness::new(&data);
    let r = h.panel.rects["Animation ruler"];
    let p = pos2(r[0] + 20., r[1] + r[3] / 2.);
    assert!(h.step(&data, pointer(p, true)).commands.is_empty());
    assert!(h
        .step(&data, vec![egui::Event::PointerMoved(p + vec2(50., 0.))])
        .commands
        .is_empty());
    assert_eq!(h.panel.scrub, Some(700.));
    let out = h.step(&data, pointer(p + vec2(50., 0.), false));
    assert_eq!(out.commands, vec![seek(700.)]);
}

#[test]
fn inspector_drag_commits_once_and_escape_keeps_committed_value() {
    let (state, _, data) = scene();
    let id = uid(&data["elements"][0]);
    let ctx = egui::Context::default();
    let mut editor = ElementInspector::default();
    let mut frame = 0;
    let mut step = |editor: &mut ElementInspector, events: Vec<egui::Event>| {
        frame += 1;
        let mut commands = vec![];
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1000., 900.))),
                time: Some(frame as f64 / 60.),
                events,
                focused: true,
                ..Default::default()
            },
            |ctx| {
                commands = editor.panel(ctx, &state, id, 1);
            },
        );
        commands
    };
    step(&mut editor, vec![]);
    step(&mut editor, vec![]);
    let p = point(editor.rects["Animation property angle"]);
    step(&mut editor, pointer(p, true));
    assert!(step(
        &mut editor,
        vec![egui::Event::PointerMoved(p - vec2(10., 0.))]
    )
    .is_empty());
    let commands = step(&mut editor, pointer(p - vec2(10., 0.), false));
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].1["prop"], "angle");
    assert!(
        commands[0].1["value"]
            .as_str()
            .unwrap()
            .parse::<f64>()
            .unwrap()
            < 360.
    );
    step(&mut editor, vec![]);
    step(&mut editor, pointer(p, true));
    step(
        &mut editor,
        vec![egui::Event::PointerMoved(p - vec2(15., 0.))],
    );
    assert!(step(&mut editor, vec![escape()]).is_empty());
    assert!(step(&mut editor, pointer(p - vec2(15., 0.), false)).is_empty());
}

struct InspectorHarness {
    ctx: egui::Context,
    frame: u64,
    editor: ElementInspector,
    state: DesktopAppState,
    camera: Camera,
    uid: u64,
    history: Vec<DesktopAppState>,
}
impl InspectorHarness {
    fn new() -> Self {
        let (state, camera, data) = scene();
        let mut h = Self {
            ctx: Default::default(),
            frame: 0,
            editor: Default::default(),
            state,
            camera,
            uid: uid(&data["elements"][0]),
            history: vec![],
        };
        h.step(vec![]);
        h.step(vec![]);
        h
    }
    fn step(&mut self, events: Vec<egui::Event>) {
        self.frame += 1;
        let mut commands = vec![];
        let _ = self.ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1000., 1000.))),
                time: Some(self.frame as f64 / 60.),
                events,
                focused: true,
                ..Default::default()
            },
            |ctx| {
                commands = self.editor.panel(ctx, &self.state, self.uid, self.frame);
            },
        );
        for (tool, args) in commands {
            let command = if tool == "edit_animation_property" {
                cuemol_command::prepare_animation_edit(&mut self.state, &mut self.camera, &args)
                    .unwrap()
            } else {
                Some((tool, args))
            };
            if let Some((tool, args)) = command {
                let before = self.state.clone();
                cuemol_command::call_tool(&mut self.state, &mut self.camera, tool, &args).unwrap();
                if !before.same_scene_undo_state(&self.state) {
                    self.history.push(before);
                }
            }
        }
    }
    fn click(&mut self, key: &str) {
        let p = point(self.editor.rects[key]);
        self.step(pointer(p, true));
        self.step(pointer(p, false));
        self.step(vec![]);
    }
    fn detail(&self) -> Value {
        cuemol_command::animation_element_detail(&self.state, self.uid).unwrap()
    }
}

#[test]
fn inspector_timing_previews_update_dependents_and_abort_without_history() {
    let mut h = InspectorHarness::new();
    let original = h.state.clone();
    let p = point(h.editor.rects["Animation property durationMs"]);
    h.step(pointer(p, true));
    h.step(vec![egui::Event::PointerMoved(p + vec2(15., 0.))]);
    assert!(h.detail()["endMs"].as_f64().unwrap() > 1000.);
    assert!(h.history.is_empty());
    let duration = h.detail()["endMs"].clone();
    assert_eq!(
        cuemol_command::animation_timeline(&h.state).unwrap()["elements"][1]["absStartMs"],
        duration
    );
    h.step(pointer(p + vec2(15., 0.), false));
    assert_eq!(h.history.len(), 1);
    assert!(original.same_scene_undo_state(&h.history[0]));
    h.step(vec![]);
    let committed = h.state.clone();
    let p = point(h.editor.rects["Animation property startMs"]);
    h.step(pointer(p, true));
    h.step(vec![egui::Event::PointerMoved(p + vec2(25., 0.))]);
    assert!(h.detail()["startMs"].as_f64().unwrap() > 0.);
    h.step(vec![escape()]);
    h.step(pointer(p + vec2(25., 0.), false));
    assert!(committed.same_scene_undo_state(&h.state));
    assert_eq!(h.history.len(), 1);
}

#[test]
fn generic_animation_numeric_preview_aborts_and_commits_once() {
    let mut h = InspectorHarness::new();
    h.click("Animation Generic tab");
    let original = h.state.clone();
    let p = point(h.editor.rects["Animation property angle"]);
    h.step(pointer(p, true));
    h.step(vec![egui::Event::PointerMoved(p - vec2(30., 0.))]);
    assert_ne!(h.detail()["properties"]["angle"], "360");
    assert!(h.history.is_empty());
    h.step(vec![escape()]);
    h.step(pointer(p - vec2(30., 0.), false));
    assert!(original.same_scene_undo_state(&h.state));
    h.step(vec![]);
    let p = point(h.editor.rects["Animation property angle"]);
    h.step(pointer(p, true));
    h.step(vec![egui::Event::PointerMoved(p - vec2(30., 0.))]);
    h.step(pointer(p - vec2(30., 0.), false));
    assert_eq!(h.history.len(), 1);
    assert!(original.same_scene_undo_state(&h.history[0]));
}

#[test]
fn spin_axis_presets_and_cartesian_components_preserve_explicit_mode() {
    let mut h = InspectorHarness::new();
    h.click("Animation axis preset");
    h.click("Animation axis x");
    let axis = cuemol_core::Vector4::from_script_string(
        h.detail()["properties"]["axis"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!([axis.x, axis.y, axis.z], [1., 0., 0.]);
    h.click("Animation axis preset");
    h.click("Animation axis cart");
    assert_eq!(h.history.len(), 1);
    let p = point(h.editor.rects["Animation axis component y"]);
    h.step(pointer(p, true));
    h.step(pointer(p, false));
    h.step(vec![
        egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        },
        egui::Event::Text("2".into()),
        egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        },
    ]);
    let axis = cuemol_core::Vector4::from_script_string(
        h.detail()["properties"]["axis"].as_str().unwrap(),
    )
    .unwrap();
    assert!((axis.y / axis.x - 2.).abs() < 1e-9);
    assert_eq!(h.history.len(), 2);
}
