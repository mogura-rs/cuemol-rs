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
        for edit in txn.edits.iter_mut().rev() {
            edit.undo();
        }
        self.nesting = 0;
        true
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
        self.disabled = true;
        let mut ok = true;
        for edit in txn.edits.iter_mut().rev() {
            if !edit.undo() {
                ok = false;
                break;
            }
        }
        self.disabled = false;
        self.redo_stack.push(txn);
        ok
    }

    pub fn redo(&mut self) -> bool {
        if !self.is_redoable() {
            return false;
        }
        let Some(mut txn) = self.redo_stack.pop() else {
            return false;
        };
        self.disabled = true;
        let mut ok = true;
        for edit in &mut txn.edits {
            if !edit.redo() {
                ok = false;
                break;
            }
        }
        self.disabled = false;
        self.undo_stack.push(txn);
        ok
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
        assert_eq!(manager.redo_size(), 1);
    }
}
