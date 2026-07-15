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
    wait_ids: Vec<TaskId>,
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

    pub fn get_queue_len(&self) -> usize {
        self.queue_len()
    }

    pub fn get_supported_scr(&self) -> &str {
        ""
    }

    pub fn error_msg(&self) -> &str {
        &self.error_msg
    }

    pub fn get_error_msg(&self) -> &str {
        self.error_msg()
    }

    pub fn set_slot_size(&mut self, slot_size: usize) {
        self.slot_size = if slot_size == 0 {
            std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
        } else {
            slot_size
        };
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
        wait_for: impl Into<String>,
    ) -> Result<TaskId> {
        self.queue_task_impl(command, args, wait_for, "")
    }

    fn queue_task_impl(
        &mut self,
        command: impl Into<String>,
        args: impl Into<String>,
        wait_for: impl Into<String>,
        work_dir: impl Into<String>,
    ) -> Result<TaskId> {
        let id = TaskId(self.next_id);
        self.next_id += 1;
        let wait_ids = wait_for
            .into()
            .split_whitespace()
            .filter_map(|value| value.parse::<u64>().ok())
            .map(TaskId::new)
            .collect();
        let work_dir = {
            let value = work_dir.into();
            (!value.trim().is_empty()).then(|| PathBuf::from(value.trim()))
        };
        self.tasks.push(Task {
            id,
            command: command.into(),
            args: args.into(),
            wait_ids,
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
        wait_for: impl Into<String>,
        cwd: impl Into<String>,
    ) -> Result<TaskId> {
        self.queue_task_impl(path, args, wait_for, cwd)
    }

    pub fn queue_task2(
        &mut self,
        path: impl Into<String>,
        args: impl Into<String>,
        wait_for: impl Into<String>,
        cwd: impl Into<String>,
    ) -> Result<TaskId> {
        self.queue_task_2(path, args, wait_for, cwd)
    }

    pub fn is_alive(&mut self, id: TaskId) -> bool {
        self.refresh_finished();
        self.task(id)
            .is_ok_and(|task| matches!(task.status, TaskStatus::Queued | TaskStatus::Running))
    }

    pub fn task_status(&mut self, id: TaskId) -> Result<TaskStatus> {
        self.refresh_finished();
        Ok(self.task(id)?.status)
    }

    pub fn get_state(&mut self, id: TaskId) -> i32 {
        self.task_status(id).map_or(3, |status| status as i32)
    }

    pub fn get_task_status(&mut self, id: TaskId) -> i32 {
        self.get_state(id)
    }

    pub fn wait_for_exit(&mut self, id: TaskId) -> Result<TaskStatus> {
        loop {
            self.refresh_finished();
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
        if let Some(index) = self
            .tasks
            .iter()
            .position(|task| task.id == id && task.status == TaskStatus::Queued)
        {
            self.tasks.remove(index);
            self.start_queued_tasks();
            return Ok(());
        }
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
        self.tasks.clear();
        self.error_msg.clear();
    }

    pub fn is_empty(&mut self) -> bool {
        self.refresh_finished();
        !self
            .tasks
            .iter()
            .any(|task| matches!(task.status, TaskStatus::Queued | TaskStatus::Running))
    }

    pub fn result_output(&self, id: TaskId) -> Result<&str> {
        Ok(&self.task_ref(id)?.output)
    }

    pub fn peek_result_output(&self, id: TaskId) -> Result<String> {
        Ok(self.result_output(id)?.to_string())
    }

    pub fn get_result_output(&mut self, id: TaskId) -> Result<String> {
        self.refresh_finished();
        let index = self
            .tasks
            .iter()
            .position(|task| task.id == id)
            .ok_or_else(|| CueMolError::NotFound(format!("task {}", id.get())))?;
        if self.tasks[index].status == TaskStatus::Ended {
            return Ok(self.tasks.remove(index).output);
        }
        Ok(self.tasks[index].output.clone())
    }

    pub fn done_task_list_json(&mut self) -> String {
        self.refresh_finished();
        let entries = self
            .tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Ended)
            .map(|task| task.id.get().to_string())
            .collect::<Vec<_>>();
        format!("[{}]", entries.join(","))
    }

    fn start_queued_tasks(&mut self) {
        while self.running_count() < self.slot_size {
            let Some(index) = (0..self.tasks.len()).find(|index| {
                let task = &self.tasks[*index];
                task.status == TaskStatus::Queued
                    && task.wait_ids.iter().all(|wait_id| {
                        !self.tasks.iter().any(|dependency| {
                            dependency.id == *wait_id
                                && matches!(
                                    dependency.status,
                                    TaskStatus::Queued | TaskStatus::Running
                                )
                        })
                    })
            }) else {
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
        let args = shlex::split(&task.args)
            .ok_or_else(|| CueMolError::invalid_argument("invalid command arguments"))?;
        command.args(args);
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
        assert_eq!(manager.done_task_list_json(), format!("[{}]", id.get()));
        assert_eq!(manager.get_result_output(id).unwrap(), "hello");
        assert_eq!(manager.done_task_list_json(), "[]");
        assert!(manager.is_empty());
        assert_eq!(manager.get_state(TaskId::new(9999)), 3);
    }

    #[test]
    fn process_manager_honors_wait_ids_and_quoted_arguments() {
        let mut manager = ProcessManager::new();
        manager.set_slot_size(2);
        let prerequisite = manager.queue_task("sleep", "0.05", "").unwrap();
        let dependent = manager
            .queue_task("printf", "'hello world'", prerequisite.get().to_string())
            .unwrap();
        assert_eq!(manager.task_status(dependent).unwrap(), TaskStatus::Queued);
        assert!(manager.is_alive(dependent));
        manager.wait_for_exit(dependent).unwrap();
        assert_eq!(manager.result_output(dependent).unwrap(), "hello world");

        manager.set_slot_size(1);
        let blocker = manager.queue_task("sleep", "0.05", "").unwrap();
        let queued = manager.queue_task("printf", "unused", "").unwrap();
        assert_eq!(manager.task_status(queued).unwrap(), TaskStatus::Queued);
        manager.kill(queued).unwrap();
        assert_eq!(manager.get_state(queued), 3);
        manager.wait_for_exit(blocker).unwrap();
    }
}
