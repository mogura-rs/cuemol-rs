use super::PythonRunOutput;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

type Handler<'a> = &'a mut dyn FnMut(Value) -> Result<Value, String>;

#[derive(Debug)]
struct Streams {
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

#[derive(Clone, Debug)]
pub struct PythonConnection(Arc<Mutex<Streams>>);

impl PythonConnection {
    pub fn same_process(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    fn send(&self, value: &Value) -> Result<(), String> {
        let mut streams = self.0.lock().map_err(|_| "Python stream lock poisoned")?;
        writeln!(streams.input, "{value}")
            .and_then(|()| streams.input.flush())
            .map_err(|error| format!("Python input failed: {error}"))
    }

    fn receive(&self) -> Result<Value, String> {
        let mut streams = self.0.lock().map_err(|_| "Python stream lock poisoned")?;
        let mut line = String::new();
        if streams
            .output
            .read_line(&mut line)
            .map_err(|error| error.to_string())?
            == 0
        {
            return Err("Python worker exited before returning a result".into());
        }
        serde_json::from_str(&line).map_err(|error| format!("invalid Python response: {error}"))
    }

    fn exchange(&self, request: Value, handler: Handler<'_>) -> Result<Value, String> {
        self.send(&request)?;
        loop {
            let result = self.receive()?;
            if let Some(request) = result.get("rpc") {
                let reply = match handler(request.clone()) {
                    Ok(value) => json!({"value": value}),
                    Err(error) => json!({"error": error}),
                };
                self.send(&reply)?;
            } else {
                return Ok(result);
            }
        }
    }

    pub fn invoke_callback(
        &self,
        id: u64,
        arguments: Value,
        handler: Handler<'_>,
    ) -> Result<(Value, PythonRunOutput), String> {
        let result = self.exchange(json!({"callback": id, "arguments": arguments}), handler)?;
        let output = run_output(&result)?;
        Ok((result["value"].clone(), output))
    }

    pub fn release_callbacks(&self, ids: &[u64], handler: Handler<'_>) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }
        let result = self.exchange(json!({"release_callbacks": ids}), handler)?;
        let output = run_output(&result)?;
        if output.success() {
            Ok(())
        } else {
            Err(output.stderr)
        }
    }
}

#[derive(Debug)]
pub(crate) struct PythonWorker {
    child: Child,
    pub(crate) connection: PythonConnection,
}

impl PythonWorker {
    pub(crate) fn start() -> Result<Self, String> {
        let helpers = json!({
            "cuemol.cuemol": include_str!("helpers/cuemol.py"),
            "cuemol.cmd": include_str!("helpers/cmd.py"),
            "cuemol.fileio": include_str!("helpers/fileio.py"),
            "cuemol.internal_loader": include_str!("helpers/internal_loader.py"),
            "cuemol.logging": include_str!("helpers/logging.py"),
            "cuemol.mol_util": include_str!("helpers/mol_util.py"),
            "cuemol.net_fetch": include_str!("helpers/net_fetch.py"),
            "cuemol.renderer": include_str!("helpers/renderer.py"),
            "cuemol.scene_util": include_str!("helpers/scene_util.py"),
            "cuemol.undo_txn": include_str!("helpers/undo_txn.py"),
            "cuemol.wrapper_base": include_str!("helpers/wrapper_base.py"),
        });
        let source = format!(
            "import json\n_helper_sources = json.loads({})\n{}\n{}",
            super::python_string_literal(&helpers.to_string()),
            include_str!("binding.py"),
            include_str!("worker.py")
        );
        let mut child = Command::new("uv")
            .args(["run", "--no-project", "python", "-u", "-c", &source])
            .env("UV_CACHE_DIR", super::uv_cache_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| format!("cannot start Python: {error}"))?;
        let input = child.stdin.take().expect("piped stdin");
        let output = BufReader::new(child.stdout.take().expect("piped stdout"));
        Ok(Self {
            child,
            connection: PythonConnection(Arc::new(Mutex::new(Streams { input, output }))),
        })
    }

    pub(crate) fn execute(
        &mut self,
        source: &str,
        handler: Handler<'_>,
    ) -> Result<PythonRunOutput, String> {
        run_output(
            &self
                .connection
                .exchange(json!({"source": source}), handler)?,
        )
    }
}

fn run_output(result: &Value) -> Result<PythonRunOutput, String> {
    let status = result["status"]
        .as_i64()
        .ok_or("missing Python exit status")?;
    Ok(PythonRunOutput {
        status_code: Some(status as i32),
        stdout: result["stdout"]
            .as_str()
            .ok_or("missing Python stdout")?
            .into(),
        stderr: result["stderr"]
            .as_str()
            .ok_or("missing Python stderr")?
            .into(),
    })
}

impl Drop for PythonWorker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
