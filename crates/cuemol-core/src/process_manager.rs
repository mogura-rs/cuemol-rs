use crate::{CueMolError, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(u64);

impl TaskId {
    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Queued = 0,
    Running = 1,
    Ended = 2,
}

#[derive(Debug)]
struct Task {
    id: TaskId,
    command: String,
    args: String,
    work_dir: Option<PathBuf>,
    status: TaskStatus,
    child: Option<Child>,
    output: String,
}

#[derive(Debug)]
pub struct ProcessManager {
    slot_size: usize,
    tasks: Vec<Task>,
    next_id: u64,
    log_path: Option<PathBuf>,
    error_msg: String,
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            slot_size: 2,
            tasks: Vec::new(),
            next_id: 1,
            log_path: None,
            error_msg: String::new(),
        }
    }

    pub fn queue_len(&self) -> usize {
        self.tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Queued)
            .count()
    }

    pub fn error_msg(&self) -> &str {
        &self.error_msg
    }

    pub fn set_slot_size(&mut self, slot_size: usize) {
        self.slot_size = slot_size.max(1);
        self.start_queued_tasks();
    }

    pub fn get_slot_size(&self) -> usize {
        self.slot_size
    }

    pub fn set_log_path(&mut self, path: impl Into<PathBuf>) {
        self.log_path = Some(path.into());
    }

    pub fn queue_task(
        &mut self,
        command: impl Into<String>,
        args: impl Into<String>,
        work_dir: impl Into<String>,
    ) -> Result<TaskId> {
        let id = TaskId(self.next_id);
        self.next_id += 1;
        let work_dir = {
            let value = work_dir.into();
            (!value.trim().is_empty()).then(|| PathBuf::from(value.trim()))
        };
        self.tasks.push(Task {
            id,
            command: command.into(),
            args: args.into(),
            work_dir,
            status: TaskStatus::Queued,
            child: None,
            output: String::new(),
        });
        self.start_queued_tasks();
        Ok(id)
    }

    pub fn queue_task_2(
        &mut self,
        path: impl Into<String>,
        args: impl Into<String>,
        _wait_for: impl Into<String>,
        cwd: impl Into<String>,
    ) -> Result<TaskId> {
        self.queue_task(path, args, cwd)
    }

    pub fn is_alive(&mut self, id: TaskId) -> bool {
        self.refresh_finished();
        self.task(id)
            .is_ok_and(|task| task.status == TaskStatus::Running)
    }

    pub fn task_status(&mut self, id: TaskId) -> Result<TaskStatus> {
        self.refresh_finished();
        Ok(self.task(id)?.status)
    }

    pub fn get_task_status(&mut self, id: TaskId) -> Result<i32> {
        Ok(self.task_status(id)? as i32)
    }

    pub fn wait_for_exit(&mut self, id: TaskId) -> Result<TaskStatus> {
        loop {
            self.start_queued_tasks();
            let status = self.task(id)?.status;
            match status {
                TaskStatus::Queued => std::thread::sleep(std::time::Duration::from_millis(10)),
                TaskStatus::Running => {
                    self.collect_running_task(id)?;
                    self.start_queued_tasks();
                }
                TaskStatus::Ended => return Ok(TaskStatus::Ended),
            }
        }
    }

    pub fn kill(&mut self, id: TaskId) -> Result<()> {
        let task = self.task_mut(id)?;
        if let Some(child) = task.child.as_mut() {
            let _ = child.kill();
        }
        self.collect_running_task(id)?;
        self.start_queued_tasks();
        Ok(())
    }

    pub fn kill_all(&mut self) {
        let ids = self.tasks.iter().map(|task| task.id).collect::<Vec<_>>();
        for id in ids {
            let _ = self.kill(id);
        }
    }

    pub fn result_output(&self, id: TaskId) -> Result<&str> {
        Ok(&self.task_ref(id)?.output)
    }

    pub fn peek_result_output(&self, id: TaskId) -> Result<String> {
        Ok(self.result_output(id)?.to_string())
    }

    pub fn get_result_output(&self, id: TaskId) -> Result<String> {
        self.peek_result_output(id)
    }

    pub fn done_task_list_json(&self) -> String {
        let entries = self
            .tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Ended)
            .map(|task| {
                format!(
                    "{{\"id\":{},\"command\":\"{}\",\"status\":2}}",
                    task.id.get(),
                    escape_json(&task.command)
                )
            })
            .collect::<Vec<_>>();
        format!("[{}]", entries.join(","))
    }

    fn start_queued_tasks(&mut self) {
        while self.running_count() < self.slot_size {
            let Some(index) = self
                .tasks
                .iter()
                .position(|task| task.status == TaskStatus::Queued)
            else {
                break;
            };
            if let Err(err) = self.start_task(index) {
                self.error_msg = err.to_string();
                self.tasks[index].status = TaskStatus::Ended;
            }
        }
    }

    fn start_task(&mut self, index: usize) -> Result<()> {
        let task = &mut self.tasks[index];
        let mut command = Command::new(&task.command);
        command.args(task.args.split_whitespace());
        if let Some(work_dir) = &task.work_dir {
            command.current_dir(work_dir);
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let child = command.spawn().map_err(CueMolError::from)?;
        task.child = Some(child);
        task.status = TaskStatus::Running;
        Ok(())
    }

    fn refresh_finished(&mut self) {
        let ids = self
            .tasks
            .iter_mut()
            .filter_map(|task| {
                if task.status != TaskStatus::Running {
                    return None;
                }
                let finished = task
                    .child
                    .as_mut()
                    .and_then(|child| child.try_wait().ok())
                    .flatten()
                    .is_some();
                finished.then_some(task.id)
            })
            .collect::<Vec<_>>();
        for id in ids {
            let _ = self.collect_running_task(id);
        }
        self.start_queued_tasks();
    }

    fn collect_running_task(&mut self, id: TaskId) -> Result<()> {
        let log_path = self.log_path.clone();
        let task = self.task_mut(id)?;
        let Some(child) = task.child.take() else {
            task.status = TaskStatus::Ended;
            return Ok(());
        };
        let output = child.wait_with_output().map_err(CueMolError::from)?;
        let mut text = String::new();
        text.push_str(&String::from_utf8_lossy(&output.stdout));
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        task.output = text;
        task.status = TaskStatus::Ended;
        if let Some(path) = log_path {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = OpenOptions::new().create(true).append(true).open(path)?;
            file.write_all(task.output.as_bytes())?;
        }
        Ok(())
    }

    fn running_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Running)
            .count()
    }

    fn task(&mut self, id: TaskId) -> Result<&mut Task> {
        self.task_mut(id)
    }

    fn task_mut(&mut self, id: TaskId) -> Result<&mut Task> {
        self.tasks
            .iter_mut()
            .find(|task| task.id == id)
            .ok_or_else(|| CueMolError::NotFound(format!("task {}", id.get())))
    }

    fn task_ref(&self, id: TaskId) -> Result<&Task> {
        self.tasks
            .iter()
            .find(|task| task.id == id)
            .ok_or_else(|| CueMolError::NotFound(format!("task {}", id.get())))
    }
}

fn escape_json(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_manager_runs_and_captures_output() {
        let mut manager = ProcessManager::new();
        manager.set_slot_size(1);
        let id = manager.queue_task("printf", "hello", "").unwrap();
        assert!(manager.is_alive(id) || manager.task_status(id).unwrap() == TaskStatus::Ended);
        manager.wait_for_exit(id).unwrap();
        assert_eq!(manager.task_status(id).unwrap(), TaskStatus::Ended);
        assert_eq!(manager.result_output(id).unwrap(), "hello");
        assert!(manager.done_task_list_json().contains("\"status\":2"));
    }
}
