use cuemol_gfx::parse_color;
use cuemol_molstr::{parse_selector, read_pdb_file, HandlerCategory, StreamManager};
use std::fs;
#[cfg(test)]
use std::process::Command;
#[cfg(not(target_arch = "wasm32"))]
mod worker;
#[cfg(not(target_arch = "wasm32"))]
pub use worker::PythonConnection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonRunRecord {
    source: PythonRunSource,
    arg1: i32,
    arg2: i32,
    label: String,
    content_len: usize,
}

impl PythonRunRecord {
    pub fn source(&self) -> &PythonRunSource {
        &self.source
    }

    pub fn arg1(&self) -> i32 {
        self.arg1
    }

    pub fn arg2(&self) -> i32 {
        self.arg2
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn content_len(&self) -> usize {
        self.content_len
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PythonRunSource {
    File(String),
    String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonRunOutput {
    status_code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl PythonRunOutput {
    pub fn status_code(&self) -> Option<i32> {
        self.status_code
    }

    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    pub fn stderr(&self) -> &str {
        &self.stderr
    }

    pub fn success(&self) -> bool {
        self.status_code == Some(0)
    }
}

#[derive(Debug, Default, Clone)]
pub struct PythonBridge {
    records: Vec<PythonRunRecord>,
    execute_scripts: bool,
    last_output: Option<PythonRunOutput>,
    #[cfg(not(target_arch = "wasm32"))]
    worker: std::sync::Arc<std::sync::Mutex<Option<worker::PythonWorker>>>,
}

impl PythonBridge {
    fn record(&mut self, record: PythonRunRecord) {
        const LIMIT: usize = 256;
        if self.records.len() >= LIMIT {
            self.records.remove(0);
        }
        self.records.push(record);
    }
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_uv_execution() -> Self {
        let mut bridge = Self::new();
        bridge.enable_uv_execution(true);
        bridge
    }

    pub fn enable_uv_execution(&mut self, enabled: bool) {
        self.execute_scripts = enabled;
    }

    pub fn uv_execution_enabled(&self) -> bool {
        self.execute_scripts
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn connection(&mut self) -> Result<PythonConnection, String> {
        if !self.execute_scripts {
            return Err("Python execution is disabled".into());
        }
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| "Python worker lock poisoned")?;
        if worker.is_none() {
            *worker = Some(worker::PythonWorker::start()?);
        }
        Ok(worker.as_ref().unwrap().connection.clone())
    }

    pub fn run_file(&mut self, path: &str) -> Result<(), String> {
        self.run_file_with_handler(path, &mut no_live_binding)
    }

    pub fn run_file_with_handler(
        &mut self,
        path: &str,
        handler: &mut dyn FnMut(serde_json::Value) -> Result<serde_json::Value, String>,
    ) -> Result<(), String> {
        let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
        self.record(PythonRunRecord {
            source: PythonRunSource::File(path.to_string()),
            arg1: 0,
            arg2: 0,
            label: String::new(),
            content_len: content.len(),
        });
        if self.execute_scripts {
            self.execute_source_with_handler(&python_file_source(path), handler)?;
        }
        Ok(())
    }

    pub fn run_file2(&mut self, path: &str, arg1: i32, arg2: i32) -> Result<(), String> {
        self.run_file3(path, arg1, arg2, "None")
    }

    pub fn run_file_2(&mut self, path: &str, arg1: i32, arg2: i32) -> Result<(), String> {
        self.run_file2(path, arg1, arg2)
    }

    pub fn run_file3(
        &mut self,
        path: &str,
        arg1: i32,
        arg2: i32,
        label: &str,
    ) -> Result<(), String> {
        self.run_file3_with_handler(path, arg1, arg2, label, &mut no_live_binding)
    }

    pub fn run_file3_with_handler(
        &mut self,
        path: &str,
        arg1: i32,
        arg2: i32,
        label: &str,
        handler: &mut dyn FnMut(serde_json::Value) -> Result<serde_json::Value, String>,
    ) -> Result<(), String> {
        let content = fs::read_to_string(path).map_err(|err| err.to_string())?;
        self.record(PythonRunRecord {
            source: PythonRunSource::File(path.to_string()),
            arg1,
            arg2,
            label: label.to_string(),
            content_len: content.len(),
        });
        if self.execute_scripts {
            self.execute_file(path, arg1, arg2, label, handler)?;
        }
        Ok(())
    }

    pub fn run_file_3(
        &mut self,
        path: &str,
        arg1: i32,
        arg2: i32,
        label: &str,
    ) -> Result<(), String> {
        self.run_file3(path, arg1, arg2, label)
    }

    pub fn run_string(&mut self, script: &str) {
        let _ = self.try_run_string(script);
    }

    pub fn try_run_string(&mut self, script: &str) -> Result<(), String> {
        self.try_run_string_with_handler(script, &mut no_live_binding)
    }

    pub fn try_run_string_with_handler(
        &mut self,
        script: &str,
        handler: &mut dyn FnMut(serde_json::Value) -> Result<serde_json::Value, String>,
    ) -> Result<(), String> {
        self.record(PythonRunRecord {
            source: PythonRunSource::String,
            arg1: 0,
            arg2: 0,
            label: String::new(),
            content_len: script.len(),
        });
        if self.execute_scripts {
            self.execute_source_with_handler(script, handler)?;
        }
        Ok(())
    }

    /// The most recent 256 calls, without source contents.
    pub fn records(&self) -> &[PythonRunRecord] {
        &self.records
    }

    pub fn last_record(&self) -> Option<&PythonRunRecord> {
        self.records.last()
    }

    pub fn last_output(&self) -> Option<&PythonRunOutput> {
        self.last_output.as_ref()
    }

    pub fn clear(&mut self) {
        self.records.clear();
        self.last_output = None;
    }

    fn execute_file(
        &mut self,
        path: &str,
        scene_id: i32,
        view_id: i32,
        arguments: &str,
        handler: &mut dyn FnMut(serde_json::Value) -> Result<serde_json::Value, String>,
    ) -> Result<(), String> {
        let arguments = if arguments.trim().is_empty() {
            "None"
        } else {
            arguments
        };
        let source = format!(
            "arguments = {arguments}\nscene_id = {scene_id}\nview_id = {view_id}\n{}",
            python_file_source(path)
        );
        self.execute_source_with_handler(&source, handler)
    }

    #[cfg(target_arch = "wasm32")]
    fn execute_source_with_handler(
        &mut self,
        _source: &str,
        _handler: &mut dyn FnMut(serde_json::Value) -> Result<serde_json::Value, String>,
    ) -> Result<(), String> {
        Err("Python execution requires a native build".into())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn execute_source_with_handler(
        &mut self,
        source: &str,
        handler: &mut dyn FnMut(serde_json::Value) -> Result<serde_json::Value, String>,
    ) -> Result<(), String> {
        let run_output = {
            let mut worker = self
                .worker
                .lock()
                .map_err(|_| "Python worker lock poisoned")?;
            if worker.is_none() {
                *worker = Some(worker::PythonWorker::start()?);
            }
            match worker.as_mut().unwrap().execute(source, handler) {
                Ok(output) => output,
                Err(error) => {
                    *worker = None;
                    return Err(error);
                }
            }
        };
        let success = run_output.success();
        let stderr = run_output.stderr.clone();
        self.last_output = Some(run_output);
        if success {
            Ok(())
        } else {
            Err(if stderr.trim().is_empty() {
                "python execution failed".to_string()
            } else {
                stderr
            })
        }
    }
}

fn no_live_binding(request: serde_json::Value) -> Result<serde_json::Value, String> {
    if request["op"] == "release" {
        Ok(serde_json::Value::Null)
    } else {
        Err("live CueMol bindings require the application scripting runtime".into())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn uv_cache_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("cuemol2-rs-uv-cache")
}

fn python_string_literal(value: &str) -> String {
    let mut literal = String::from("'");
    for ch in value.chars() {
        match ch {
            '\\' => literal.push_str("\\\\"),
            '\'' => literal.push_str("\\'"),
            '\n' => literal.push_str("\\n"),
            '\r' => literal.push_str("\\r"),
            '\t' => literal.push_str("\\t"),
            ch => literal.push(ch),
        }
    }
    literal.push('\'');
    literal
}

fn python_file_source(path: &str) -> String {
    format!(
        "__file__ = {}\nwith open(__file__, 'rb') as _cuemol_script_file:\n    exec(compile(_cuemol_script_file.read(), __file__, 'exec'), globals(), globals())\ndel _cuemol_script_file\n",
        python_string_literal(path)
    )
}

pub fn compile_selection_dump(expression: &str) -> Result<String, String> {
    parse_selector(expression)
        .map(|selector| selector.dump())
        .map_err(|err| err.to_string())
}

pub fn color_syntax_is_valid(expression: &str) -> bool {
    parse_color(expression).is_ok()
}

pub fn pdb_array_metadata(path: &str, array_name: &str) -> Result<String, String> {
    let mol = read_pdb_file(path).map_err(|err| err.to_string())?;
    let array = match array_name {
        "crd" => mol.crd_array(),
        "bfac" | "occ" => mol.prop_array(array_name),
        other => return Err(format!("unknown molecule array: {other}")),
    };
    Ok(format!(
        "{{\"type\":{},\"length\":{},\"bytes\":{}}}",
        array.elem_type_code(),
        array.elem_count(),
        array.byte_len()
    ))
}

pub fn pdb_summary_json(path: &str) -> Result<String, String> {
    let mol = read_pdb_file(path).map_err(|err| err.to_string())?;
    Ok(format!(
        "{{\"atoms\":{},\"residues\":{},\"chains\":{},\"chainNames\":{}}}",
        mol.atom_count(),
        mol.residue_count(),
        mol.chain_ids().len(),
        mol.chains_json()
    ))
}

pub fn pdb_selection_count(path: &str, expression: &str) -> Result<usize, String> {
    let mol = read_pdb_file(path).map_err(|err| err.to_string())?;
    let selector = parse_selector(expression).map_err(|err| err.to_string())?;
    Ok(mol.atom_sel_count(&selector))
}

pub fn search_reader_by_content(
    path: &str,
    nicknames: &str,
    category: i32,
    support_compression: bool,
    max_bytes: u64,
) -> Result<String, String> {
    let category = HandlerCategory::from_code(category).map_err(|err| err.to_string())?;
    Ok(StreamManager::new().search_reader_by_content(
        path,
        nicknames,
        category,
        support_compression,
        max_bytes,
    ))
}

pub fn search_readers_by_content(
    path: &str,
    nicknames: &str,
    category: i32,
    support_compression: bool,
    max_bytes: u64,
) -> Result<String, String> {
    let category = HandlerCategory::from_code(category).map_err(|err| err.to_string())?;
    Ok(StreamManager::new().search_readers_by_content(
        path,
        nicknames,
        category,
        support_compression,
        max_bytes,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_binding_ready_core_functions() {
        assert_eq!(compile_selection_dump("elem C,N").unwrap(), "elem N,C");
        assert!(!color_syntax_is_valid("Rgb(255, 0, 0)"));
    }

    #[test]
    fn exposes_pdb_array_metadata_for_bindings() {
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let path = root.join("1CRN.pdb");
        assert_eq!(
            pdb_array_metadata(path.to_str().unwrap(), "crd").unwrap(),
            "{\"type\":21,\"length\":981,\"bytes\":3924}"
        );
    }

    #[test]
    fn exposes_pdb_summary_selection_and_stream_search_for_bindings() {
        let Some(root) = reference_test_data_root() else {
            return;
        };
        let path = root.join("1CRN.pdb");
        assert_eq!(
            pdb_summary_json(path.to_str().unwrap()).unwrap(),
            "{\"atoms\":327,\"residues\":46,\"chains\":1,\"chainNames\":[\"A\"]}"
        );
        assert_eq!(
            pdb_selection_count(path.to_str().unwrap(), "chain A and name CA").unwrap(),
            46
        );
        assert_eq!(
            search_reader_by_content(
                path.to_str().unwrap(),
                "",
                HandlerCategory::OBJ_READER,
                false,
                0
            )
            .unwrap(),
            "pdb"
        );
    }

    #[test]
    fn python_bridge_records_qif_run_requests() {
        let mut bridge = PythonBridge::new();
        bridge.run_string("print('ok')");
        assert_eq!(bridge.records().len(), 1);
        assert_eq!(
            bridge.last_record().unwrap().source(),
            &PythonRunSource::String
        );

        let path = std::env::temp_dir().join("cuemol-py-bridge-test.py");
        std::fs::write(&path, "x = 1\n").unwrap();
        bridge
            .run_file3(path.to_str().unwrap(), 2, 3, "startup")
            .unwrap();
        let record = bridge.last_record().unwrap();
        assert_eq!(
            record.source(),
            &PythonRunSource::File(path.to_string_lossy().into())
        );
        assert_eq!(record.arg1(), 2);
        assert_eq!(record.arg2(), 3);
        assert_eq!(record.label(), "startup");
        assert_eq!(record.content_len(), 6);
        bridge.clear();
        assert!(bridge.records().is_empty());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn python_bridge_can_execute_with_uv_when_available() {
        if !uv_python_available() {
            return;
        }
        let mut bridge = PythonBridge::with_uv_execution();
        bridge
            .try_run_string("print('bridge-ok')\n")
            .expect("uv python should run string");
        assert_eq!(bridge.last_output().unwrap().stdout(), "bridge-ok\n");
        bridge.try_run_string("import os, subprocess, sys\nprint('python-output')\nsubprocess.run([sys.executable, '-c', \"import sys; print('child-output'); print('child-error', file=sys.stderr)\"], check=True)\nos.write(1, b'native-output\\n')\n").unwrap();
        assert_eq!(
            bridge.last_output().unwrap().stdout(),
            "python-output\nchild-output\nnative-output\n"
        );
        assert_eq!(bridge.last_output().unwrap().stderr(), "child-error\n");

        let path = std::env::temp_dir().join("cuemol-py-bridge-exec-test.py");
        std::fs::write(&path, "print(f'{scene_id}:{view_id}:{arguments[0]}')\n").unwrap();
        bridge
            .run_file3(path.to_str().unwrap(), 10, 20, "['arg']")
            .expect("uv python should run file");
        assert_eq!(bridge.last_output().unwrap().stdout(), "10:20:arg\n");
        bridge.try_run_string("values = [1]\n").unwrap();
        let mut clone = bridge.clone();
        clone.try_run_string("values.append(2)\n").unwrap();
        assert!(bridge.try_run_string("raise ValueError('probe')").is_err());
        bridge
            .try_run_string("assert values == [1, 2]\nprint(values)\n")
            .unwrap();
        assert_eq!(bridge.last_output().unwrap().stdout(), "[1, 2]\n");
        std::fs::write(
            &path,
            "assert __name__ == '__main__'\nfile_value = values[1] + scene_id\nprint(__file__)\n",
        )
        .unwrap();
        bridge.run_file(path.to_str().unwrap()).unwrap();
        assert_eq!(
            bridge.last_output().unwrap().stdout().trim(),
            path.to_str().unwrap()
        );
        bridge.try_run_string("assert file_value == 12\n").unwrap();
        let _ = std::fs::remove_file(path);
    }

    fn reference_test_data_root() -> Option<std::path::PathBuf> {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2")
            });
        let path = root.join("tests/test_data");
        path.exists().then_some(path)
    }

    fn uv_python_available() -> bool {
        let output = Command::new("uv")
            .arg("run")
            .arg("--no-project")
            .arg("python")
            .arg("-c")
            .arg("import sys; sys.exit(0)")
            .env("UV_CACHE_DIR", uv_cache_dir())
            .output();
        matches!(output, Ok(output) if output.status.success())
    }
}
