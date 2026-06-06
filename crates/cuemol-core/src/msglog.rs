use crate::{LString, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct LMsgLog {
    accum: String,
    accum_enabled: bool,
    file_redir_path: PathBuf,
}

impl LMsgLog {
    pub const DL_ERROR: i32 = 0;
    pub const DL_WARN: i32 = 10;
    pub const DL_NOTIFY: i32 = 20;
    pub const DL_VERBOSE: i32 = 30;

    pub fn new() -> Self {
        Self {
            accum_enabled: true,
            ..Self::default()
        }
    }

    pub fn write_log(&mut self, level: i32, message: &str) -> Result<()> {
        self.write_log_with_redir(level, message, false)
    }

    pub fn write_log_with_redir(
        &mut self,
        level: i32,
        message: &str,
        force_file: bool,
    ) -> Result<()> {
        if self.accum_enabled && level <= Self::DL_WARN {
            self.accum.push_str(message);
            self.accum.push('\n');
        }
        if force_file || !self.file_redir_path.as_os_str().is_empty() {
            self.append_file(message)?;
        }
        Ok(())
    }

    pub fn write_err(&mut self, message: &str) {
        if self.accum_enabled {
            self.accum.push_str(message);
        }
    }

    pub fn write_err_ln(&mut self, message: &str) {
        if self.accum_enabled {
            self.accum.push_str(message);
            self.accum.push('\n');
        }
    }

    pub fn writeln(&mut self, message: &str) -> Result<()> {
        self.write_log(Self::DL_NOTIFY, message)
    }

    pub fn accum_msg(&self) -> LString {
        LString::from(self.accum.clone())
    }

    pub fn get_accum_msg(&self) -> LString {
        self.accum_msg()
    }

    pub fn remove_accum_msg(&mut self) {
        self.accum.clear();
        self.accum_enabled = false;
    }

    pub fn enable_accum_msg(&mut self) {
        self.accum_enabled = true;
    }

    pub fn file_redir_path(&self) -> LString {
        LString::from(self.file_redir_path.to_string_lossy().to_string())
    }

    pub fn get_file_redir_path(&self) -> LString {
        self.file_redir_path()
    }

    pub fn set_file_redir_path(&mut self, path: impl AsRef<Path>) {
        self.file_redir_path = path.as_ref().to_path_buf();
    }

    fn append_file(&self, message: &str) -> Result<()> {
        if self.file_redir_path.as_os_str().is_empty() {
            return Ok(());
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_redir_path)?;
        writeln!(file, "{message}")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msglog_accumulation_and_remove_match_reference() {
        let mut log = LMsgLog::new();
        log.write_log(LMsgLog::DL_WARN, "warn message").unwrap();
        assert!(log.accum_msg().index_of("warn message") >= 0);
        log.write_log(LMsgLog::DL_ERROR, "err message").unwrap();
        assert!(log.accum_msg().index_of("err message") >= 0);
        log.write_log(LMsgLog::DL_NOTIFY, "notify_sentinel_xyz")
            .unwrap();
        assert!(log.accum_msg().index_of("notify_sentinel_xyz") < 0);

        log.write_err("error text");
        log.write_err_ln("error line");
        assert!(log.accum_msg().index_of("error text") >= 0);
        assert!(log.accum_msg().index_of("error line") >= 0);
        log.remove_accum_msg();
        assert!(log.accum_msg().is_empty());
        log.write_log(LMsgLog::DL_ERROR, "after remove").unwrap();
        assert!(log.accum_msg().is_empty());
    }

    #[test]
    fn msglog_file_redirection_writes_messages() {
        let path = std::env::temp_dir().join(format!("cuemol-msglog-{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let mut log = LMsgLog::new();
        assert!(log.file_redir_path().is_empty());
        log.set_file_redir_path(&path);
        assert!(log.file_redir_path().ends_with(".txt"));
        log.write_log_with_redir(LMsgLog::DL_NOTIFY, "redir test", true)
            .unwrap();
        log.set_file_redir_path("");
        assert!(log.file_redir_path().is_empty());

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("redir test"));
        let _ = std::fs::remove_file(path);
    }
}
