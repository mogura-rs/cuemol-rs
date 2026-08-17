use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

static NEXT_POV_JOB_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) fn create_pov_work_dir() -> Result<PathBuf, String> {
    let serial = NEXT_POV_JOB_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("cuemol-pov-{}-{serial}", std::process::id()));
    fs::create_dir(&path)
        .map_err(|error| format!("failed to create POV-Ray working directory: {error}"))?;
    Ok(path)
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PovProcessOptions {
    pub width: u32,
    pub height: u32,
    pub threads: u32,
    pub stereo: i32,
    pub stereo_depth: f64,
    pub perspective: bool,
    pub transparent: bool,
    pub shadow: bool,
    pub light_spread: u32,
    pub light_intensity: f64,
    pub flash_fraction: f64,
    pub ambient_fraction: f64,
    pub radiosity: i32,
    pub verbose: bool,
}

impl Default for PovProcessOptions {
    fn default() -> Self {
        Self {
            width: 500,
            height: 500,
            threads: 1,
            stereo: 0,
            stereo_depth: 0.03,
            perspective: false,
            transparent: false,
            shadow: false,
            light_spread: 1,
            light_intensity: 1.3,
            flash_fraction: 0.8 / 1.3,
            ambient_fraction: 0.0,
            radiosity: -1,
            verbose: true,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PovProcessTask {
    pub executable: String,
    pub include_dir: String,
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub options: PovProcessOptions,
}

#[derive(Debug)]
pub(crate) struct PovProcessOutput {
    pub output_path: PathBuf,
    pub log: String,
}

pub(crate) fn povray_args(task: &PovProcessTask) -> Vec<String> {
    let options = &task.options;
    let mut args = vec![
        format!("+I{}", task.input_path.to_string_lossy()),
        format!("+O{}", task.output_path.to_string_lossy()),
        format!("+W{}", options.width.max(1)),
        format!("+H{}", options.height.max(1)),
        "+FN8".to_string(),
        "-D".to_string(),
        format!("+WT{}", options.threads.clamp(1, 32)),
        "Quality=11".to_string(),
        "Antialias=On".to_string(),
        "Antialias_Depth=3".to_string(),
        "Antialias_Threshold=0.1".to_string(),
        "Jitter=Off".to_string(),
        "File_Gamma=1".to_string(),
        format!("Declare=_stereo={}", options.stereo.clamp(-1, 1)),
        format!("Declare=_iod={}", options.stereo_depth.clamp(0.0, 1.0)),
        format!("Declare=_perspective={}", u8::from(options.perspective)),
        format!("Declare=_shadow={}", u8::from(options.shadow)),
        format!("Declare=_light_inten={}", options.light_intensity),
        format!("Declare=_flash_frac={}", options.flash_fraction),
        format!("Declare=_amb_frac={}", options.ambient_fraction),
    ];
    if let Some(input_dir) = task.input_path.parent() {
        args.push(format!("+L{}", input_dir.to_string_lossy()));
    }
    if !task.include_dir.trim().is_empty() {
        args.push(format!("+L{}", task.include_dir.trim()));
    }
    if options.light_spread > 1 {
        args.push(format!(
            "Declare=_light_spread={}",
            options.light_spread.clamp(1, 10)
        ));
    }
    if options.transparent {
        args.push("+UA".to_string());
        args.push("Declare=_transpbg=1".to_string());
        args.push("Declare=_no_fog=1".to_string());
    }
    if options.radiosity >= 0 {
        args.push(format!("Declare=_radiosity={}", options.radiosity));
    }
    args.push(if options.verbose { "+V" } else { "-V" }.to_string());
    args
}

pub(crate) fn run_pov_process(
    task: PovProcessTask,
    cancel: Arc<AtomicBool>,
) -> Result<PovProcessOutput, String> {
    run_pov_process_with_poll(task, cancel, || {})
}

pub(crate) fn run_pov_process_with_poll(
    task: PovProcessTask,
    cancel: Arc<AtomicBool>,
    mut poll: impl FnMut(),
) -> Result<PovProcessOutput, String> {
    if task.executable.trim().is_empty() {
        return Err("POV-Ray executable path is required".to_string());
    }
    validate_options(&task.options)?;
    if !task.input_path.is_file() {
        return Err(format!(
            "POV-Ray input file does not exist: {}",
            task.input_path.display()
        ));
    }
    if task.output_path.exists() {
        fs::remove_file(&task.output_path).map_err(|error| {
            format!(
                "failed to replace POV-Ray output {}: {error}",
                task.output_path.display()
            )
        })?;
    }
    let work_dir = task
        .input_path
        .parent()
        .ok_or_else(|| "POV-Ray input path has no parent directory".to_string())?;
    let mut child = Command::new(task.executable.trim())
        .args(povray_args(&task))
        .current_dir(work_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to start POV-Ray ({}): {error}", task.executable))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture POV-Ray output".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture POV-Ray errors".to_string())?;
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
            Ok(None) => {
                poll();
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(format!("failed while waiting for POV-Ray: {error}"));
            }
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| "POV-Ray output reader failed".to_string())?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "POV-Ray error reader failed".to_string())?;
    if status.is_none() {
        return Err("POV-Ray rendering cancelled".to_string());
    }
    let status = status.expect("cancelled status was handled above");
    let mut log = String::from_utf8_lossy(&stdout).into_owned();
    log.push_str(&String::from_utf8_lossy(&stderr));
    if !status.success() {
        let detail = log.trim();
        return Err(if detail.is_empty() {
            format!("POV-Ray exited with {status}")
        } else {
            format!("POV-Ray exited with {status}: {detail}")
        });
    }
    match fs::metadata(&task.output_path) {
        Ok(metadata) if metadata.len() > 0 => Ok(PovProcessOutput {
            output_path: task.output_path,
            log,
        }),
        _ => Err("POV-Ray did not produce an image".to_string()),
    }
}

fn validate_options(options: &PovProcessOptions) -> Result<(), String> {
    if options.width == 0 || options.height == 0 {
        return Err("POV-Ray image dimensions must be positive".to_string());
    }
    for (name, value) in [
        ("stereo depth", options.stereo_depth),
        ("light intensity", options.light_intensity),
        ("flash fraction", options.flash_fraction),
        ("ambient fraction", options.ambient_fraction),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(format!("POV-Ray {name} must be finite and non-negative"));
        }
    }
    if options.stereo_depth > 1.0 || options.flash_fraction > 1.0 || options.ambient_fraction > 1.0
    {
        return Err("POV-Ray stereo depth and lighting fractions must not exceed one".to_string());
    }
    Ok(())
}

fn read_process_stream(mut stream: impl Read) -> Vec<u8> {
    let mut output = Vec::new();
    let _ = stream.read_to_end(&mut output);
    output
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn runs_povray_with_reference_options_and_collects_output() {
        let dir =
            std::env::temp_dir().join(format!("cuemol-pov-process-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let input_path = dir.join("render.pov");
        let output_path = dir.join("render.png");
        let args_path = dir.join("args.txt");
        let executable = dir.join("povray.sh");
        fs::write(&input_path, "camera {}\n").unwrap();
        fs::write(
            &executable,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"{}\"\nprintf 'PNG' > \"{}\"\nprintf 'Rendered 1 of 1 pixels (100%%)\\n'\n",
                args_path.display(),
                output_path.display()
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();

        let output = run_pov_process(
            PovProcessTask {
                executable: executable.to_string_lossy().into_owned(),
                include_dir: "/tmp/pov include".to_string(),
                input_path,
                output_path: output_path.clone(),
                options: PovProcessOptions {
                    width: 800,
                    height: 600,
                    threads: 4,
                    stereo: -1,
                    stereo_depth: 0.05,
                    perspective: true,
                    transparent: true,
                    shadow: true,
                    light_spread: 3,
                    radiosity: 7,
                    ..PovProcessOptions::default()
                },
            },
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert_eq!(output.output_path, output_path);
        assert!(output.log.contains("100%"));
        let args = fs::read_to_string(args_path).unwrap();
        for expected in [
            "+W800",
            "+H600",
            "+WT4",
            "Declare=_stereo=-1",
            "Declare=_iod=0.05",
            "Declare=_perspective=1",
            "Declare=_shadow=1",
            "Declare=_light_spread=3",
            "Declare=_radiosity=7",
            "Declare=_transpbg=1",
            "+L/tmp/pov include",
        ] {
            assert!(
                args.lines().any(|argument| argument == expected),
                "{expected}"
            );
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cancels_a_running_povray_process() {
        let dir =
            std::env::temp_dir().join(format!("cuemol-pov-cancel-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let input_path = dir.join("render.pov");
        let output_path = dir.join("render.png");
        let executable = dir.join("povray.sh");
        fs::write(&input_path, "camera {}\n").unwrap();
        fs::write(&executable, "#!/bin/sh\nexec sleep 10\n").unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let worker = thread::spawn(move || {
            run_pov_process(
                PovProcessTask {
                    executable: executable.to_string_lossy().into_owned(),
                    include_dir: String::new(),
                    input_path,
                    output_path,
                    options: PovProcessOptions::default(),
                },
                worker_cancel,
            )
        });
        thread::sleep(Duration::from_millis(100));
        cancel.store(true, Ordering::Relaxed);
        assert_eq!(
            worker.join().unwrap().unwrap_err(),
            "POV-Ray rendering cancelled"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
