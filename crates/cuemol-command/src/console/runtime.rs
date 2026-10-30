use super::{split_commands, Action, Dialect, Session};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use std::collections::VecDeque;

pub(super) fn outside_transaction(name: &str, args: &serde_json::Value) -> bool {
    matches!(name, "save_scene" | "clear_undo" | "undo" | "redo")
        || name == "load_file"
            && args["path"]
                .as_str()
                .is_some_and(|path| path.trim().to_ascii_lowercase().ends_with(".qsc"))
}

#[derive(Debug)]
enum Work {
    Line {
        dialect: Dialect,
        text: String,
        quiet: bool,
        depth: usize,
    },
    Action {
        dialect: Dialect,
        action: Action,
        depth: usize,
    },
}

#[derive(Debug)]
pub struct Submission {
    work: VecDeque<Work>,
    standalone: bool,
}

impl Submission {
    pub fn new(source: &str, dialect: Dialect) -> Result<Self, String> {
        let mut submission = Self {
            work: VecDeque::new(),
            standalone: dialect != Dialect::JavaScript && split_commands(source)?.len() == 1,
        };
        submission.prepend(source, dialect, 0)?;
        Ok(submission)
    }

    fn prepend(&mut self, source: &str, dialect: Dialect, depth: usize) -> Result<(), String> {
        if depth > 8 {
            return Err("Console scripts cannot be nested more than eight levels".into());
        }
        if dialect == Dialect::JavaScript {
            self.work.push_front(Work::Line {
                dialect,
                text: source.into(),
                quiet: false,
                depth,
            });
        } else {
            for command in split_commands(source)?.into_iter().rev() {
                self.work.push_front(Work::Line {
                    dialect,
                    text: command.text,
                    quiet: command.quiet,
                    depth,
                });
            }
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.work.is_empty()
    }

    pub fn next(
        &mut self,
        session: &mut Session,
        state: &DesktopAppState,
        camera: &Camera,
    ) -> Result<Option<Action>, String> {
        loop {
            match self.work.pop_front() {
                None => return Ok(None),
                Some(Work::Line {
                    dialect,
                    text,
                    quiet,
                    depth,
                }) => {
                    let actions = session.plan_dialect(state, camera, dialect, &text)?;
                    for action in actions.into_iter().rev() {
                        self.work.push_front(Work::Action {
                            dialect,
                            action,
                            depth,
                        });
                    }
                    if !quiet {
                        let word = super::parser::head(&text).0;
                        if depth == 0
                            && !["log", "log_open", "log_close", "open_log", "close_log"]
                                .contains(&word)
                        {
                            if let Err(error) = session.log_line(&text) {
                                self.work.push_front(Work::Action {
                                    dialect,
                                    action: Action::Output(error),
                                    depth,
                                });
                            }
                        }
                        return Ok(Some(Action::Echo(format!("{} {text}", dialect.prompt()))));
                    }
                }
                Some(Work::Action {
                    dialect,
                    action: Action::Source(source),
                    depth,
                }) => {
                    self.prepend(&source, dialect, depth + 1)?;
                }
                Some(Work::Action {
                    action: Action::Native(source),
                    depth,
                    ..
                }) => {
                    self.prepend(&source, Dialect::CueMol, depth + 1)?;
                }
                Some(Work::Action {
                    action: Action::Qax(source),
                    dialect,
                    depth,
                }) => {
                    if dialect != Dialect::CueMol {
                        return Err("Switch to CueMol to run a QAX file".into());
                    }
                    if depth >= 8 {
                        return Err(
                            "Console scripts cannot be nested more than eight levels".into()
                        );
                    }
                    for action in super::qax::actions(&source, depth == 0 && self.standalone)?
                        .into_iter()
                        .rev()
                    {
                        self.work.push_front(Work::Action {
                            dialect,
                            action,
                            depth: depth + 1,
                        });
                    }
                }
                Some(Work::Action {
                    action: Action::QaxOp(node),
                    ..
                }) => {
                    return session.qax_op(state, &node).map(Some);
                }
                Some(Work::Action {
                    action: Action::KeepChain { name, chain },
                    ..
                }) => {
                    let object = state
                        .active_object_summaries()
                        .into_iter()
                        .rev()
                        .find(|object| object.name == name)
                        .ok_or_else(|| format!("No molecule named {name}"))?;
                    let selector = state
                        .parse_scene_selector(
                            state.active_scene().ok_or("No active scene")?,
                            &format!("chain {chain}"),
                        )
                        .map_err(|e| e.to_string())?;
                    if state
                        .scene_manager()
                        .object(object.uid)
                        .unwrap()
                        .mol()
                        .select_indices(&selector)
                        .is_empty()
                    {
                        return Err(format!("No such chain: {chain}"));
                    }
                    return Ok(Some(Action::Tool {
                        name: "delete_atoms".into(),
                        args: serde_json::json!({"objId":object.uid.get(),"selection":format!("not (chain {chain})")}),
                    }));
                }
                Some(Work::Action {
                    action: Action::RememberSetting { scene, name, value },
                    ..
                }) => {
                    let key = (scene, name);
                    if let Some(value) = value {
                        session.remembered_settings.insert(key, value);
                    } else {
                        session.remembered_settings.remove(&key);
                    }
                }
                Some(Work::Action { action, depth, .. }) => {
                    let (action, outside) = match action {
                        Action::Tool { name, args } => {
                            let outside = outside_transaction(&name, &args);
                            (Action::Tool { name, args }, outside)
                        }
                        action => (action, false),
                    };
                    if outside && (depth != 0 || !self.standalone) {
                        return Err(
                            "This command must be submitted alone, outside scripts and QAX actions"
                                .into(),
                        );
                    }
                    return Ok(Some(action));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_core::Uid;

    #[test]
    fn scene_io_and_history_reset_cannot_split_a_submission() {
        let mut state = super::super::tests::state();
        let mut camera = Camera::new();
        let mut session = Session::default();
        let mut run = Submission::new("hide protein; clear_undo", Dialect::CueMol).unwrap();
        loop {
            match run.next(&mut session, &state, &camera) {
                Ok(Some(Action::Tool { name, args })) => {
                    assert_ne!(name, "clear_undo");
                    crate::call_tool(&mut state, &mut camera, &name, &args).unwrap();
                }
                Ok(Some(_)) => {}
                Err(error) => {
                    assert!(error.contains("submitted alone"));
                    break;
                }
                Ok(None) => panic!("The history reset must be rejected"),
            }
        }
        assert!(!state.active_object_summaries()[0].visible);
        for source in ["save out.qsc; count_atoms", "count_atoms; save out.qsc"] {
            let mut run = Submission::new(source, Dialect::PyMol).unwrap();
            loop {
                match run.next(&mut session, &state, &camera) {
                    Ok(Some(Action::Tool { name, .. })) if name == "save_scene" => {
                        panic!("Must not save inside a transaction")
                    }
                    Ok(Some(_)) => {}
                    Err(error) => {
                        assert!(error.contains("submitted alone"));
                        break;
                    }
                    Ok(None) => panic!("The scene save must be rejected"),
                }
            }
        }
        let mut run = Submission::new("save out.qsc", Dialect::PyMol).unwrap();
        assert!(matches!(
            run.next(&mut session, &state, &camera).unwrap(),
            Some(Action::Echo(_))
        ));
        assert!(matches!(
            run.next(&mut session, &state, &camera).unwrap(),
            Some(Action::Tool { name, .. }) if name == "save_scene"
        ));
        assert!(outside_transaction(
            "load_file",
            &serde_json::json!({"path":"scene.QSC"})
        ));
        assert!(!outside_transaction(
            "load_file",
            &serde_json::json!({"path":"protein.pdb"})
        ));
    }

    #[test]
    fn command_names_are_resolved_after_each_completed_mutation() {
        let mut state = DesktopAppState::new();
        state.open_bytes("probe.pdb", b"ATOM      1  CA  GLY A   1       1.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        let uid = state.active_object_summaries()[0].uid;
        let source = format!("rename #{}, renamed; create_renderer renamed, simple, name=new; set renamed/new.visible, false", uid.get());
        let mut run = Submission::new(&source, Dialect::CueMol).unwrap();
        let mut session = Session::default();
        let mut camera = Camera::new();
        while let Some(action) = run.next(&mut session, &state, &camera).unwrap() {
            if let Action::Tool { name, args } = action {
                crate::call_tool(&mut state, &mut camera, &name, &args).unwrap();
            }
        }
        assert_eq!(state.active_object_summaries()[0].name, "renamed");
        assert!(
            !state
                .renderer_summaries(Uid::new(uid.get()))
                .iter()
                .find(|r| r.name == "new")
                .unwrap()
                .visible
        );
    }
}
