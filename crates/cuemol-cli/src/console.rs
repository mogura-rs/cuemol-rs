use cuemol_command::console::{Action, Dialect, Session, Submission};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use std::io::{BufRead, IsTerminal, Write};
type Result<T> = std::result::Result<T, String>;

pub(crate) fn dialect(value: &str) -> Result<Dialect> {
    match value.to_ascii_lowercase().as_str() {
        "cuemol" => Ok(Dialect::CueMol),
        "pymol" => Ok(Dialect::PyMol),
        "chimerax" | "chx" => Ok(Dialect::ChimeraX),
        "javascript" | "js" => Ok(Dialect::JavaScript),
        _ => Err(format!("Unknown dialect: {value}")),
    }
}
pub(crate) fn script(args: &[String]) -> Result<Option<(Dialect, usize)>> {
    if args.first().is_some_and(|a| a == "--dialect") {
        return Ok(Some((
            dialect(args.get(1).ok_or("--dialect requires a name")?)?,
            2,
        )));
    }
    let kind = args
        .first()
        .and_then(|p| std::path::Path::new(p).extension())
        .and_then(|e| e.to_str());
    Ok(match kind.map(str::to_ascii_lowercase).as_deref() {
        Some("cxc") => Some((Dialect::ChimeraX, 0)),
        Some("pml") => Some((Dialect::PyMol, 0)),
        _ => None,
    })
}
pub fn run(args: Vec<String>) -> Result<()> {
    if args
        .first()
        .is_some_and(|a| matches!(a.as_str(), "-h" | "--help"))
    {
        println!("cuemol console [--dialect cuemol|pymol|chimerax|javascript] [-c COMMANDS | SCRIPT]\nWithout a command or script, read submissions from standard input.");
        return Ok(());
    }
    let (kind, at) = if args.first().is_some_and(|a| a == "--dialect") {
        (dialect(args.get(1).ok_or("--dialect requires a name")?)?, 2)
    } else {
        (Dialect::CueMol, 0)
    };
    match &args[at..] {
        [flag, source] if flag == "-c" || flag == "--eval" => execute(source, kind),
        [path] => execute(
            &std::fs::read_to_string(path).map_err(|e| e.to_string())?,
            kind,
        ),
        [] => {
            let mut runtime = Runtime::new()?;
            let terminal = std::io::stdin().is_terminal();
            let mut input = std::io::stdin().lock();
            loop {
                if terminal {
                    print!("{} ", kind.prompt());
                    std::io::stdout().flush().map_err(|e| e.to_string())?;
                }
                let mut line = String::new();
                if input.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                    break;
                }
                match runtime.submit(&line, kind) {
                    Ok(false) => break,
                    Ok(true) => {}
                    Err(error) if terminal => eprintln!("{error}"),
                    Err(error) => return Err(error),
                }
            }
            Ok(())
        }
        _ => Err("Usage: cuemol console [--dialect NAME] [-c COMMANDS | SCRIPT]".into()),
    }
}
pub fn execute(source: &str, dialect: Dialect) -> Result<()> {
    Runtime::new()?.submit(source, dialect).map(|_| ())
}
struct Runtime {
    state: DesktopAppState,
    camera: Camera,
    session: Session,
    undo: Vec<DesktopAppState>,
    redo: Vec<DesktopAppState>,
}
impl Runtime {
    fn new() -> Result<Self> {
        let (state, camera) = crate::runtime::state()?;
        Ok(Self {
            state,
            camera,
            session: Session::default(),
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }
    fn submit(&mut self, source: &str, dialect: Dialect) -> Result<bool> {
        let mut submission = Submission::new(source, dialect)?;
        self.session.begin_submission(source, dialect);
        let mut before = None;
        let result = (|| {
            while let Some(action) =
                submission.next(&mut self.session, &self.state, &self.camera)?
            {
                match action {
                    Action::Tool { name, args } => {
                        if matches!(name.as_str(), "undo" | "redo") {
                            let (from, to) = if name == "undo" {
                                (&mut self.undo, &mut self.redo)
                            } else {
                                (&mut self.redo, &mut self.undo)
                            };
                            let mut restored =
                                from.pop().ok_or_else(|| format!("Nothing to {name}"))?;
                            restored.preserve_global_styles_from(&self.state);
                            self.camera =
                                restored.preserve_view_state_from(&self.state, &self.camera);
                            to.push(self.state.replace_snapshot(restored));
                            self.session.scene_changed();
                            continue;
                        }
                        if cuemol_command::tool_changes_scene(&name)
                            && cuemol_command::tool_records_history(&name)
                            && before.is_none()
                        {
                            before = Some(self.state.clone());
                        }
                        self.session.prepare_record(&self.state, &name, &args);
                        let result = cuemol_command::call_tool(
                            &mut self.state,
                            &mut self.camera,
                            &name,
                            &args,
                        )
                        .map_err(|e| e.to_string())?;
                        self.session.complete_record(
                            !result.is_null() && cuemol_command::tool_is_mutating(&name),
                        );
                        if cuemol_command::tool_is_mutating(&name) {
                            self.session.scene_changed();
                        }
                        let output = cuemol_command::console::format_output(&result);
                        if !output.is_empty() {
                            println!("{output}");
                        }
                        if matches!(name.as_str(), "save_scene" | "clear_undo" | "load_file")
                            && (name != "load_file"
                                || args["path"]
                                    .as_str()
                                    .is_some_and(|p| p.to_ascii_lowercase().ends_with(".qsc")))
                        {
                            self.undo.clear();
                            self.redo.clear();
                            before = None;
                        }
                    }
                    Action::Output(text) => println!("{text}"),
                    Action::Echo(_) => {}
                    Action::Transaction(label) => {
                        self.session.begin_transaction(&label)?;
                        self.finish_undo(before.take());
                    }
                    Action::WriteFile { path, bytes, .. } => {
                        std::fs::write(path, bytes.as_ref()).map_err(|e| e.to_string())?
                    }
                    Action::Render {
                        width,
                        height,
                        ray,
                        dpi,
                        path,
                    } => {
                        let name = if ray {
                            "render_image"
                        } else {
                            "get_screenshot"
                        };
                        let args = serde_json::json!({"width":width,"height":height,"fileName":"","style":if ray{Some("gi")}else{None}});
                        let mut prepared =
                            cuemol_command::prepare_render(&self.state, &self.camera, name, &args)
                                .map_err(|e| e.to_string())?
                                .in_memory();
                        if let Some(dpi) = dpi {
                            prepared = prepared.with_dpi(dpi);
                        }
                        let rendered = prepared.run().map_err(|e| e.to_string())?;
                        let bytes = if ray {
                            self.session
                                .remember_render(&self.state, &self.camera, &rendered)?
                                .as_ref()
                                .clone()
                        } else {
                            use base64::Engine;
                            base64::engine::general_purpose::STANDARD
                                .decode(
                                    rendered["_image"]
                                        .as_str()
                                        .ok_or("Rendered image missing")?,
                                )
                                .map_err(|e| e.to_string())?
                        };
                        if let Some(path) = path {
                            std::fs::write(path, bytes).map_err(|e| e.to_string())?;
                        }
                    }
                    Action::Clear => {}
                    Action::Quit => return Ok(false),
                    Action::Source(_)
                    | Action::Native(_)
                    | Action::ChimeraX(_)
                    | Action::Qax(_)
                    | Action::QaxOp(_)
                    | Action::RememberSetting { .. }
                    | Action::KeepChain { .. } => unreachable!("Submission expands this action"),
                }
            }
            Ok(true)
        })();
        self.finish_undo(before);
        let finished = self.session.finish_submission();
        result.and_then(|keep| finished.map(|_| keep))
    }
    fn finish_undo(&mut self, before: Option<DesktopAppState>) {
        if let Some(before) = before.filter(|s| !s.same_scene_undo_state(&self.state)) {
            self.undo.push(before);
            self.redo.clear();
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
        }
    }
}
