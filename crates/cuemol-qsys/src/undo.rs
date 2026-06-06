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
        if txn.is_empty() {
            return true;
        }
        self.redo_stack.clear();
        self.undo_stack.push(txn);
        true
    }

    pub fn commit_undo_txn(&mut self) -> bool {
        self.commit_txn()
    }

    pub fn rollback_txn(&mut self) -> bool {
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
        let mut ok = true;
        for edit in txn.edits.iter_mut().rev() {
            ok &= edit.undo();
        }
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
        let mut ok = true;
        for edit in &mut txn.edits {
            ok &= edit.redo();
        }
        self.undo_stack.push(txn);
        ok
    }

    pub fn clear_all_info(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.active = None;
        self.nesting = 0;
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
