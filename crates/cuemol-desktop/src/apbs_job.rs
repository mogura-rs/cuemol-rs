use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::{GuiApbsChargeMethod, GuiApbsPaths};

static NEXT_APBS_JOB_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub(crate) struct ApbsProcessTask {
    pub apbs_executable: String,
    pub pdb2pqr_executable: String,
    pub force_field: String,
    pub charge_method: GuiApbsChargeMethod,
    pub paths: GuiApbsPaths,
    pub work_dir: PathBuf,
}

#[derive(Debug)]
pub(crate) struct ApbsProcessOutput {
    pub work_dir: PathBuf,
    pub dx_path: PathBuf,
    pub log: String,
}

pub(crate) fn create_apbs_work_dir() -> Result<PathBuf, String> {
    let serial = NEXT_APBS_JOB_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("cuemol-apbs-{}-{serial}", std::process::id()));
    fs::create_dir(&path)
        .map_err(|error| format!("failed to create APBS working directory: {error}"))?;
    Ok(path)
}

pub(crate) fn run_apbs_process(
    task: ApbsProcessTask,
    cancel: Arc<AtomicBool>,
) -> Result<ApbsProcessOutput, String> {
    let result = run_apbs_process_inner(&task, &cancel);
    if result.is_err() {
        let _ = fs::remove_dir_all(&task.work_dir);
    }
    result
}

fn run_apbs_process_inner(
    task: &ApbsProcessTask,
    cancel: &Arc<AtomicBool>,
) -> Result<ApbsProcessOutput, String> {
    if task.apbs_executable.trim().is_empty() {
        return Err("APBS executable path is required".to_string());
    }
    let mut log = String::new();
    if task.charge_method == GuiApbsChargeMethod::Pdb2pqr {
        if task.pdb2pqr_executable.trim().is_empty() {
            return Err("pdb2pqr executable path is required".to_string());
        }
        let force_field = task.force_field.trim().to_ascii_uppercase();
        let mut args = if cfg!(target_os = "windows") {
            vec![force_field]
        } else {
            vec![
                "--keep-chain".to_string(),
                "--nodebump".to_string(),
                "--noopt".to_string(),
                "--ff".to_string(),
                force_field,
            ]
        };
        args.push(task.paths.pdb_path.to_string_lossy().into_owned());
        args.push(task.paths.pqr_path.to_string_lossy().into_owned());
        log.push_str(&run_command_cancelable(
            "pdb2pqr",
            task.pdb2pqr_executable.trim(),
            &args,
            &task.work_dir,
            cancel,
        )?);
        ensure_nonempty_file(&task.paths.pqr_path, "pdb2pqr did not produce a PQR file")?;
    }

    log.push_str(&run_command_cancelable(
        "APBS",
        task.apbs_executable.trim(),
        &[task.paths.input_path.to_string_lossy().into_owned()],
        &task.work_dir,
        cancel,
    )?);
    ensure_nonempty_file(
        &task.paths.dx_path,
        "APBS did not produce an electrostatic-potential map",
    )?;
    Ok(ApbsProcessOutput {
        work_dir: task.work_dir.clone(),
        dx_path: task.paths.dx_path.clone(),
        log,
    })
}

fn run_command_cancelable(
    label: &str,
    executable: &str,
    args: &[String],
    work_dir: &std::path::Path,
    cancel: &Arc<AtomicBool>,
) -> Result<String, String> {
    let mut child = Command::new(executable)
        .args(args)
        .current_dir(work_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to start {label} ({executable}): {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("failed to capture {label} output"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("failed to capture {label} errors"))?;
    let stdout_reader = thread::spawn(move || read_process_stream(stdout));
    let stderr_reader = thread::spawn(move || read_process_stream(stderr));

    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(format!("failed while waiting for {label}: {error}"));
            }
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| format!("{label} output reader failed"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| format!("{label} error reader failed"))?;
    if status.is_none() {
        return Err("APBS calculation cancelled".to_string());
    }
    let status = status.expect("cancelled status was handled above");
    let mut output = String::from_utf8_lossy(&stdout).into_owned();
    output.push_str(&String::from_utf8_lossy(&stderr));
    if !status.success() {
        let detail = output.trim();
        return Err(if detail.is_empty() {
            format!("{label} exited with {status}")
        } else {
            format!("{label} exited with {status}: {detail}")
        });
    }
    Ok(output)
}

fn read_process_stream(mut stream: impl Read) -> Vec<u8> {
    let mut output = Vec::new();
    let _ = stream.read_to_end(&mut output);
    output
}

fn ensure_nonempty_file(path: &std::path::Path, message: &str) -> Result<(), String> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.len() > 0 => Ok(()),
        _ => Err(message.to_string()),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn runs_pdb2pqr_then_apbs_and_collects_output() {
        let work_dir = create_apbs_work_dir().unwrap();
        let paths = GuiApbsPaths {
            pdb_path: work_dir.join("input.pdb"),
            pqr_path: work_dir.join("output.pqr"),
            input_path: work_dir.join("apbs.in"),
            dx_path: work_dir.join("result.dx"),
        };
        fs::write(&paths.pdb_path, "ATOM\n").unwrap();
        fs::write(&paths.input_path, "input\n").unwrap();
        let pdb2pqr = work_dir.join("pdb2pqr.sh");
        fs::write(
            &pdb2pqr,
            "#!/bin/sh\ncp \"$6\" \"$7\"\nprintf 'pdb2pqr complete\\n'\n",
        )
        .unwrap();
        let apbs = work_dir.join("apbs.sh");
        fs::write(
            &apbs,
            format!(
                "#!/bin/sh\nprintf 'object 1 class gridpositions counts 1 1 1\\n' > \"{}\"\nprintf 'apbs complete\\n'\n",
                paths.dx_path.display()
            ),
        )
        .unwrap();
        for script in [&pdb2pqr, &apbs] {
            let mut permissions = fs::metadata(script).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(script, permissions).unwrap();
        }

        let output = run_apbs_process(
            ApbsProcessTask {
                apbs_executable: apbs.to_string_lossy().into_owned(),
                pdb2pqr_executable: pdb2pqr.to_string_lossy().into_owned(),
                force_field: "charmm".to_string(),
                charge_method: GuiApbsChargeMethod::Pdb2pqr,
                paths,
                work_dir: work_dir.clone(),
            },
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert!(output.log.contains("pdb2pqr complete"));
        assert!(output.log.contains("apbs complete"));
        assert!(output.dx_path.exists());
        fs::remove_dir_all(work_dir).unwrap();
    }

    #[test]
    fn cancels_a_running_apbs_process() {
        let work_dir = create_apbs_work_dir().unwrap();
        let paths = GuiApbsPaths {
            pdb_path: work_dir.join("input.pdb"),
            pqr_path: work_dir.join("input.pqr"),
            input_path: work_dir.join("apbs.in"),
            dx_path: work_dir.join("result.dx"),
        };
        fs::write(&paths.pqr_path, "ATOM\n").unwrap();
        fs::write(&paths.input_path, "input\n").unwrap();
        let apbs = work_dir.join("slow-apbs.sh");
        fs::write(&apbs, "#!/bin/sh\nexec sleep 10\n").unwrap();
        let mut permissions = fs::metadata(&apbs).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&apbs, permissions).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let worker = thread::spawn(move || {
            run_apbs_process(
                ApbsProcessTask {
                    apbs_executable: apbs.to_string_lossy().into_owned(),
                    pdb2pqr_executable: String::new(),
                    force_field: "charmm".to_string(),
                    charge_method: GuiApbsChargeMethod::Internal,
                    paths,
                    work_dir,
                },
                worker_cancel,
            )
        });
        thread::sleep(Duration::from_millis(100));
        cancel.store(true, Ordering::Relaxed);
        let error = worker.join().unwrap().unwrap_err();
        assert_eq!(error, "APBS calculation cancelled");
    }
}
