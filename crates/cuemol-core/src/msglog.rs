use crate::event::{EventCaster, ListenerId, SharedEventListener};
use crate::{LString, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LLogEvent {
    message: LString,
    event_type: i32,
    newline: bool,
}

impl LLogEvent {
    pub fn new(event_type: i32, newline: bool, message: impl Into<LString>) -> Self {
        Self {
            message: message.into(),
            event_type,
            newline,
        }
    }

    pub fn message(&self) -> &LString {
        &self.message
    }

    pub fn event_type(&self) -> i32 {
        self.event_type
    }

    pub fn is_newline(&self) -> bool {
        self.newline
    }

    pub fn get_json(&self) -> LString {
        LString::from(format!(
            "{{ \"newline\": {}, \"content\": \"{}\" }}",
            self.newline,
            escape_json_string(self.message.as_str())
        ))
    }
}

pub struct LMsgLog {
    accum: String,
    accum_enabled: bool,
    file_redir_path: PathBuf,
    redir_initialized: bool,
    event_caster: EventCaster<LLogEvent>,
}

impl Default for LMsgLog {
    fn default() -> Self {
        Self::new()
    }
}

impl LMsgLog {
    pub const DL_ERROR: i32 = 0;
    pub const DL_WARN: i32 = 10;
    pub const DL_NOTIFY: i32 = 20;
    pub const DL_VERBOSE: i32 = 30;

    pub fn new() -> Self {
        Self {
            accum: String::new(),
            accum_enabled: true,
            file_redir_path: PathBuf::new(),
            redir_initialized: false,
            event_caster: EventCaster::new(),
        }
    }

    pub fn write_log(&mut self, level: i32, message: &str) -> Result<()> {
        self.write_log_with_redir(level, message, false)
    }

    pub fn write_log_with_redir(&mut self, level: i32, message: &str, newline: bool) -> Result<()> {
        self.write_log_nl(level, message, newline)
    }

    pub fn write_log_nl(&mut self, level: i32, message: &str, newline: bool) -> Result<()> {
        if !self.file_redir_path.as_os_str().is_empty() {
            self.append_file(message, newline)?;
        }
        if level > Self::DL_WARN {
            return Ok(());
        }
        if self.accum_enabled {
            self.accum.push_str(message);
            if newline {
                self.accum.push('\n');
            }
            return Ok(());
        }

        let mut event = LLogEvent::new(level, newline, LString::from(message));
        self.event_caster.locked_fire(&mut event);
        Ok(())
    }

    pub fn write_err(&mut self, message: &str) {
        let _ = self.write_log_nl(Self::DL_ERROR, message, false);
    }

    pub fn write_err_ln(&mut self, message: &str) {
        let _ = self.write_log_nl(Self::DL_ERROR, message, true);
    }

    pub fn write(&mut self, message: &str) -> Result<()> {
        self.write_log_nl(Self::DL_ERROR, message, false)
    }

    pub fn writeln(&mut self, message: &str) -> Result<()> {
        self.write_log_nl(Self::DL_ERROR, message, true)
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

    pub fn add_listener(&self, listener: SharedEventListener<LLogEvent>) -> Option<ListenerId> {
        self.event_caster.add(listener)
    }

    pub fn remove_listener(&self, id: ListenerId) -> bool {
        self.event_caster.remove(id).is_some()
    }

    pub fn listener_count(&self) -> usize {
        self.event_caster.listener_count()
    }

    pub fn file_redir_path(&self) -> LString {
        LString::from(self.file_redir_path.to_string_lossy().to_string())
    }

    pub fn get_file_redir_path(&self) -> LString {
        self.file_redir_path()
    }

    pub fn set_file_redir_path(&mut self, path: impl AsRef<Path>) {
        self.file_redir_path = path.as_ref().to_path_buf();
        self.redir_initialized = false;
    }

    fn append_file(&mut self, message: &str, newline: bool) -> Result<()> {
        if self.file_redir_path.as_os_str().is_empty() {
            return Ok(());
        }
        let mut options = OpenOptions::new();
        options.create(true).write(true);
        if self.redir_initialized {
            options.append(true);
        } else {
            options.truncate(true);
        }
        let mut file = options.open(&self.file_redir_path)?;
        file.write_all(message.as_bytes())?;
        if newline {
            file.write_all(b"\n")?;
        }
        file.flush()?;
        self.redir_initialized = true;
        Ok(())
    }
}

fn escape_json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                write!(escaped, "\\u{:04x}", character as u32).unwrap();
            }
            character => escaped.push(character),
        }
    }
    escaped
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

    #[test]
    fn msglog_notifies_low_level_messages_after_accumulation_is_removed() {
        use crate::shared_event_listener;
        use std::sync::{Arc, Mutex};

        let mut log = LMsgLog::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let listener_events = Arc::clone(&events);
        let listener = shared_event_listener(move |event: &mut LLogEvent| {
            listener_events.lock().unwrap().push(event.clone());
        });
        let id = log.add_listener(listener).unwrap();

        log.write_log_nl(LMsgLog::DL_WARN, "accumulated", true)
            .unwrap();
        assert!(events.lock().unwrap().is_empty());
        log.remove_accum_msg();
        log.write_log_nl(LMsgLog::DL_ERROR, "first", false).unwrap();
        log.write_log_nl(LMsgLog::DL_WARN, "second", true).unwrap();
        log.write_log_nl(LMsgLog::DL_NOTIFY, "ignored", true)
            .unwrap();

        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].message().as_str(), "first");
        assert_eq!(events[0].event_type(), LMsgLog::DL_ERROR);
        assert!(!events[0].is_newline());
        assert!(events[1].is_newline());
        drop(events);
        assert!(log.remove_listener(id));
        assert_eq!(log.listener_count(), 0);
    }

    #[test]
    fn log_event_json_escapes_content() {
        let event = LLogEvent::new(LMsgLog::DL_WARN, true, LString::from("a\"b\\c\n"));
        assert_eq!(
            event.get_json().as_str(),
            "{ \"newline\": true, \"content\": \"a\\\"b\\\\c\\n\" }"
        );
    }
}
