use std::io::{self, Write};

const MARKDOWN: &str = include_str!("../../../skills/cuemol/SKILL.md");

pub fn run(args: Vec<String>) -> Result<(), String> {
    let output = match args.as_slice() {
        [] => MARKDOWN,
        [flag] if flag == "--help" || flag == "-h" => {
            "Usage: cuemol skill\n\nPrint the bundled agent skill as Markdown.\nSave it with: cuemol skill > SKILL.md\n"
        }
        _ => return Err("Usage: cuemol skill [--help]".into()),
    };
    match io::stdout().lock().write_all(output.as_bytes()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}
