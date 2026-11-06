use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

pub trait EditInfo {
    fn undo(&mut self) -> bool;
    fn redo(&mut self) -> bool;

    fn is_undoable(&self) -> bool {
        true
    }

    fn is_redoable(&self) -> bool {
        true
    }
}

struct Transaction {
    description: String,
    edits: Vec<Box<dyn EditInfo>>,
}

impl Transaction {
    fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            edits: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }
}

#[derive(Default)]
pub struct UndoManager {
    undo_stack: Vec<Transaction>,
    redo_stack: Vec<Transaction>,
    active: Option<Transaction>,
    nesting: usize,
    disabled: bool,
    history_dropped: bool,
}

#[derive(Clone)]
pub struct UndoManagerHandle(Rc<RefCell<UndoManager>>);

impl Default for UndoManagerHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoManagerHandle {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(UndoManager::new())))
    }

    pub fn with<R>(&self, function: impl FnOnce(&UndoManager) -> R) -> R {
        function(&self.0.borrow())
    }

    pub fn with_mut<R>(&self, function: impl FnOnce(&mut UndoManager) -> R) -> R {
        function(&mut self.0.borrow_mut())
    }
}

impl fmt::Debug for UndoManagerHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("UndoManagerHandle")
            .field(&Rc::as_ptr(&self.0))
            .finish()
    }
}

impl PartialEq for UndoManagerHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for UndoManagerHandle {}

impl UndoManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_undoable(&self) -> bool {
        self.undo_stack
            .last()
            .is_some_and(|txn| txn.edits.iter().all(|edit| edit.is_undoable()))
    }

    pub fn is_redoable(&self) -> bool {
        self.redo_stack
            .last()
            .is_some_and(|txn| txn.edits.iter().all(|edit| edit.is_redoable()))
    }

    pub fn is_in_txn(&self) -> bool {
        self.active.is_some()
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }

    pub fn is_ok(&self) -> bool {
        self.is_in_txn() && !self.disabled
    }

    pub fn undo_size(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn get_undo_size(&self) -> usize {
        self.undo_size()
    }

    pub fn redo_size(&self) -> usize {
        self.redo_stack.len()
    }

    pub fn get_redo_size(&self) -> usize {
        self.redo_size()
    }

    pub fn start_txn(&mut self, description: impl Into<String>) {
        if self.disabled {
            return;
        }
        if self.active.is_none() {
            self.active = Some(Transaction::new(description));
        }
        self.nesting += 1;
    }

    pub fn start_undo_txn(&mut self, description: impl Into<String>) {
        self.start_txn(description);
    }

    pub fn add_edit_info(&mut self, edit: Box<dyn EditInfo>) -> bool {
        if self.disabled {
            return false;
        }
        let Some(active) = self.active.as_mut() else {
            return false;
        };
        active.edits.push(edit);
        true
    }

    pub fn commit_txn(&mut self) -> bool {
        if self.disabled {
            return false;
        }
        if self.nesting == 0 {
            return false;
        }
        self.nesting -= 1;
        if self.nesting != 0 {
            return true;
        }
        let Some(txn) = self.active.take() else {
            return false;
        };
        self.redo_stack.clear();
        if txn.is_empty() {
            return true;
        }
        self.undo_stack.push(txn);
        true
    }

    pub fn commit_undo_txn(&mut self) -> bool {
        self.commit_txn()
    }

    fn run_step(&mut self, txn: &mut Transaction, undo: bool) -> bool {
        let was_disabled = std::mem::replace(&mut self.disabled, true);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if undo {
                txn.edits.iter_mut().rev().all(|edit| edit.undo())
            } else {
                txn.edits.iter_mut().all(|edit| edit.redo())
            }
        }))
        .unwrap_or(false);
        self.disabled = was_disabled;
        result
    }

    fn drop_history(&mut self) {
        self.clear_all_info();
        self.history_dropped = true;
    }

    pub fn take_history_dropped(&mut self) -> bool {
        std::mem::take(&mut self.history_dropped)
    }

    pub fn rollback_txn(&mut self) -> bool {
        if self.disabled {
            return false;
        }
        if self.nesting > 1 {
            self.nesting -= 1;
            return true;
        }
        let Some(mut txn) = self.active.take() else {
            return false;
        };
        self.nesting = 0;
        let result = self.run_step(&mut txn, true);
        if !result {
            self.drop_history();
        }
        result
    }

    pub fn rollback_undo_txn(&mut self) -> bool {
        self.rollback_txn()
    }

    pub fn undo(&mut self) -> bool {
        if !self.is_undoable() {
            return false;
        }
        let Some(mut txn) = self.undo_stack.pop() else {
            return false;
        };
        if !self.run_step(&mut txn, true) {
            self.drop_history();
            return false;
        }
        self.redo_stack.push(txn);
        true
    }

    pub fn redo(&mut self) -> bool {
        if !self.is_redoable() {
            return false;
        }
        let Some(mut txn) = self.redo_stack.pop() else {
            return false;
        };
        if !self.run_step(&mut txn, false) {
            self.drop_history();
            return false;
        }
        self.undo_stack.push(txn);
        true
    }

    pub fn undo_n(&mut self, index: usize) -> bool {
        for _ in 0..=index {
            if !self.undo() {
                return false;
            }
        }
        true
    }

    pub fn redo_n(&mut self, index: usize) -> bool {
        for _ in 0..=index {
            if !self.redo() {
                return false;
            }
        }
        true
    }

    pub fn clear_all_info(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn clear_undo_data(&mut self) {
        self.clear_all_info();
    }

    pub fn undo_desc(&self, index: usize) -> Option<&str> {
        self.undo_stack
            .iter()
            .rev()
            .nth(index)
            .map(|txn| txn.description.as_str())
    }

    pub fn get_undo_desc(&self, index: usize) -> Option<&str> {
        self.undo_desc(index)
    }

    pub fn get_redo_desc(&self, index: usize) -> Option<&str> {
        self.redo_stack
            .iter()
            .rev()
            .nth(index)
            .map(|txn| txn.description.as_str())
    }

    pub fn undo_desc_list(&self) -> Vec<&str> {
        self.undo_stack
            .iter()
            .rev()
            .take_while(|txn| txn.edits.iter().all(|edit| edit.is_undoable()))
            .map(|txn| txn.description.as_str())
            .collect()
    }

    pub fn redo_desc_list(&self) -> Vec<&str> {
        self.redo_stack
            .iter()
            .rev()
            .take_while(|txn| txn.edits.iter().all(|edit| edit.is_redoable()))
            .map(|txn| txn.description.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct CountEditInfo {
        undo_count: usize,
        redo_count: usize,
    }

    impl EditInfo for CountEditInfo {
        fn undo(&mut self) -> bool {
            self.undo_count += 1;
            true
        }

        fn redo(&mut self) -> bool {
            self.redo_count += 1;
            true
        }
    }

    struct RecordingEditInfo {
        name: &'static str,
        calls: Rc<RefCell<Vec<String>>>,
        undoable: bool,
        redoable: bool,
        undo_result: bool,
        redo_result: bool,
    }

    impl RecordingEditInfo {
        fn successful(name: &'static str, calls: Rc<RefCell<Vec<String>>>) -> Self {
            Self {
                name,
                calls,
                undoable: true,
                redoable: true,
                undo_result: true,
                redo_result: true,
            }
        }
    }

    impl EditInfo for RecordingEditInfo {
        fn undo(&mut self) -> bool {
            self.calls.borrow_mut().push(format!("undo:{}", self.name));
            self.undo_result
        }

        fn redo(&mut self) -> bool {
            self.calls.borrow_mut().push(format!("redo:{}", self.name));
            self.redo_result
        }

        fn is_undoable(&self) -> bool {
            self.undoable
        }

        fn is_redoable(&self) -> bool {
            self.redoable
        }
    }

    #[test]
    fn undo_manager_default_state_matches_reference() {
        let manager = UndoManager::default();
        assert!(!manager.is_undoable());
        assert!(!manager.is_redoable());
        assert!(!manager.is_in_txn());
        assert!(!manager.is_disabled());
        assert_eq!(manager.undo_size(), 0);
        assert_eq!(manager.redo_size(), 0);
    }

    #[test]
    fn undo_manager_commits_undo_redo_and_clears_redo() {
        let mut manager = UndoManager::default();
        manager.start_txn("op1");
        assert!(manager.is_in_txn());
        assert!(manager.add_edit_info(Box::new(CountEditInfo::default())));
        assert!(manager.commit_txn());
        assert!(manager.is_undoable());
        assert_eq!(manager.undo_size(), 1);
        assert_eq!(manager.undo_desc(0), Some("op1"));

        assert!(manager.undo());
        assert!(!manager.is_undoable());
        assert!(manager.is_redoable());
        assert_eq!(manager.redo_size(), 1);

        manager.start_txn("op2");
        manager.add_edit_info(Box::new(CountEditInfo::default()));
        manager.commit_txn();
        assert_eq!(manager.redo_size(), 0);
    }

    #[test]
    fn nested_transaction_commits_only_at_outer_level() {
        let mut manager = UndoManager::default();
        manager.start_txn("outer");
        manager.start_txn("inner");
        manager.add_edit_info(Box::new(CountEditInfo::default()));
        manager.commit_txn();
        assert!(manager.is_in_txn());
        manager.commit_txn();
        assert!(!manager.is_in_txn());
        assert!(manager.is_undoable());
    }

    #[test]
    fn nested_rollback_waits_for_the_outer_transaction_and_reverses_edits() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut manager = UndoManager::new();
        manager.start_txn("outer");
        manager.add_edit_info(Box::new(RecordingEditInfo::successful(
            "first",
            Rc::clone(&calls),
        )));
        manager.start_txn("inner");
        manager.add_edit_info(Box::new(RecordingEditInfo::successful(
            "second",
            Rc::clone(&calls),
        )));

        assert!(manager.rollback_txn());
        assert!(manager.is_in_txn());
        assert!(calls.borrow().is_empty());
        assert!(manager.rollback_txn());
        assert!(!manager.is_in_txn());
        assert_eq!(
            calls.borrow().as_slice(),
            &["undo:second".to_string(), "undo:first".to_string()]
        );
    }

    #[test]
    fn empty_commit_discards_redo_and_clear_preserves_pending_transaction() {
        let mut manager = UndoManager::new();
        manager.start_txn("recorded");
        manager.add_edit_info(Box::new(CountEditInfo::default()));
        assert!(manager.commit_txn());
        assert!(manager.undo());
        assert_eq!(manager.redo_size(), 1);

        manager.start_txn("empty");
        assert!(manager.commit_txn());
        assert_eq!(manager.redo_size(), 0);

        manager.start_txn("pending");
        manager.clear_all_info();
        assert!(manager.is_in_txn());
        assert!(manager.commit_txn());
    }

    #[test]
    fn multi_step_undo_redo_and_description_lists_follow_stack_order() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut manager = UndoManager::new();
        for name in ["one", "two"] {
            manager.start_txn(name);
            manager.add_edit_info(Box::new(RecordingEditInfo::successful(
                name,
                Rc::clone(&calls),
            )));
            manager.commit_txn();
        }
        assert_eq!(manager.undo_desc_list(), vec!["two", "one"]);
        assert!(manager.undo_n(1));
        assert_eq!(manager.redo_desc_list(), vec!["one", "two"]);
        assert!(manager.redo_n(1));
        assert_eq!(
            calls.borrow().as_slice(),
            &[
                "undo:two".to_string(),
                "undo:one".to_string(),
                "redo:one".to_string(),
                "redo:two".to_string(),
            ]
        );
    }

    #[test]
    fn failed_edit_stops_transaction_execution_and_reenables_recording() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut manager = UndoManager::new();
        manager.start_txn("failure");
        manager.add_edit_info(Box::new(RecordingEditInfo::successful(
            "skipped",
            Rc::clone(&calls),
        )));
        let mut failing = RecordingEditInfo::successful("failing", Rc::clone(&calls));
        failing.undo_result = false;
        manager.add_edit_info(Box::new(failing));
        manager.commit_txn();

        assert!(!manager.undo());
        assert_eq!(calls.borrow().as_slice(), &["undo:failing".to_string()]);
        assert!(!manager.is_disabled());
        assert_eq!(manager.redo_size(), 0);
        assert_eq!(manager.undo_size(), 0);
    }
    struct PartialFailure {
        changed: Rc<std::cell::Cell<usize>>,
        fail_undo: bool,
        panics: bool,
    }

    impl PartialFailure {
        fn change(&self, fail: bool) -> bool {
            self.changed.set(self.changed.get() + 1);
            assert!(
                !(fail && self.panics),
                "edit failed after changing the scene"
            );
            !fail
        }
    }

    impl EditInfo for PartialFailure {
        fn undo(&mut self) -> bool {
            self.change(self.fail_undo)
        }
        fn redo(&mut self) -> bool {
            self.change(!self.fail_undo)
        }
    }

    #[test]
    fn failed_undo_redo_and_rollback_discard_history_and_resume_recording() {
        for which in ["undo", "redo", "rollback"] {
            for panics in [false, true] {
                let changed = Rc::new(std::cell::Cell::new(0));
                let mut manager = UndoManager::new();
                manager.start_txn("older");
                manager.add_edit_info(Box::new(CountEditInfo::default()));
                assert!(manager.commit_txn());
                if which == "rollback" {
                    assert!(manager.undo());
                }
                manager.start_txn("partial failure");
                manager.add_edit_info(Box::new(PartialFailure {
                    changed: Rc::clone(&changed),
                    fail_undo: which != "redo",
                    panics,
                }));
                if which != "rollback" {
                    assert!(manager.commit_txn());
                    if which == "redo" {
                        assert!(manager.undo());
                    } else {
                        manager.start_txn("future");
                        manager.add_edit_info(Box::new(CountEditInfo::default()));
                        assert!(manager.commit_txn());
                        assert!(manager.undo());
                    }
                }
                let before = changed.get();
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match which {
                        "undo" => manager.undo(),
                        "redo" => manager.redo(),
                        _ => manager.rollback_txn(),
                    }));
                assert!(matches!(result, Ok(false)), "{which}, panics={panics}");
                assert_eq!(changed.get(), before + 1);
                assert_eq!(
                    (manager.undo_size(), manager.redo_size()),
                    (0, 0),
                    "{which}"
                );
                assert!(!manager.is_disabled());
                assert!(!manager.is_in_txn());
                assert!(manager.take_history_dropped());
                assert!(!manager.take_history_dropped());
                manager.start_txn("later");
                manager.add_edit_info(Box::new(CountEditInfo::default()));
                assert!(manager.commit_txn());
                assert!(manager.undo());
            }
        }
    }

    #[test]
    fn refusing_an_unavailable_step_keeps_history() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut manager = UndoManager::new();
        manager.start_txn("refused");
        let mut edit = RecordingEditInfo::successful("refused", Rc::clone(&calls));
        edit.undoable = false;
        manager.add_edit_info(Box::new(edit));
        assert!(manager.commit_txn());
        assert!(!manager.undo());
        assert_eq!(manager.undo_size(), 1);
        assert!(calls.borrow().is_empty());
        assert!(!manager.take_history_dropped());
    }
}
