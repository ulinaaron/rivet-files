//! Undo / redo for file actions.
//!
//! Every finished action is recorded as a `Step`. Undoing runs the opposite action
//! (copies and new items go to the Trash, moves go back, trashed items are restored,
//! renames are reversed); the result of that is recorded on the other stack, so redo
//! simply undoes the undo. Permanent deletion is never recorded: it can't be undone.

use std::path::PathBuf;

use crate::fsops::OpKind;

#[derive(Clone, Debug)]
pub enum Step {
    /// Copies that were made (paste / drag with Ctrl / dropped from another app).
    Copied(Vec<PathBuf>),
    /// (from, to) for every moved item.
    Moved(Vec<(PathBuf, PathBuf)>),
    /// (original, where it sits in the Trash).
    Trashed(Vec<(PathBuf, PathBuf)>),
    /// (where it sat in the Trash, where it was restored to).
    Restored(Vec<(PathBuf, PathBuf)>),
    /// (old path, new path).
    Renamed(PathBuf, PathBuf),
    /// A bulk rename: (old path, new path) for every item.
    RenamedMany(Vec<(PathBuf, PathBuf)>),
    /// A new folder or document.
    Created(PathBuf),
}

fn count(n: usize) -> String {
    if n == 1 { String::new() } else { format!(" ({n} items)") }
}

impl Step {
    /// What the step was, for the Edit menu ("Undo Move to Trash").
    pub fn describe(&self) -> String {
        match self {
            Step::Copied(v) => format!("Copy{}", count(v.len())),
            Step::Moved(v) => format!("Move{}", count(v.len())),
            Step::Trashed(v) => format!("Move to Trash{}", count(v.len())),
            Step::Restored(v) => format!("Restore{}", count(v.len())),
            Step::RenamedMany(v) => format!("Rename{}", count(v.len())),
            Step::Renamed(_, to) => format!("Rename to “{}”", to.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
            Step::Created(p) => format!("Create “{}”", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
        }
    }

    /// The step a finished background action amounts to (None if nothing to undo).
    pub fn from_op(kind: OpKind, done: Vec<(PathBuf, PathBuf)>) -> Option<Step> {
        if done.is_empty() {
            return None;
        }
        Some(match kind {
            OpKind::Copy => Step::Copied(done.into_iter().map(|(_, c)| c).collect()),
            OpKind::Move => Step::Moved(done),
            OpKind::Trash => Step::Trashed(done),
            OpKind::Restore => Step::Restored(done),
            // Undoing a compress or extract moves what it made to the Trash.
            OpKind::Compress | OpKind::Extract | OpKind::Convert | OpKind::Resize => {
                let mut made: Vec<PathBuf> = done.into_iter().map(|(_, m)| m).collect();
                if made.len() == 1 { Step::Created(made.remove(0)) } else { Step::Copied(made) }
            }
            OpKind::Delete | OpKind::EmptyTrash => return None,
        })
    }
}

#[derive(Default)]
pub struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
}

const LIMIT: usize = 100;

impl History {
    /// Something you just did: it can be undone, and earlier redos are dropped.
    pub fn record(&mut self, step: Step) {
        self.undo.push(step);
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// The result of running an undo (role 1) or a redo (role 2).
    pub fn record_role(&mut self, step: Step, role: u8) {
        match role {
            1 => self.redo.push(step),
            2 => self.undo.push(step),
            _ => self.record(step),
        }
    }

    pub fn next_undo(&self) -> Option<&Step> {
        self.undo.last()
    }
    pub fn next_redo(&self) -> Option<&Step> {
        self.redo.last()
    }
    pub fn pop_undo(&mut self) -> Option<Step> {
        self.undo.pop()
    }
    pub fn pop_redo(&mut self) -> Option<Step> {
        self.redo.pop()
    }

    /// After emptying the Trash, anything that pointed into it is gone for good.
    pub fn forget_trash(&mut self) {
        let in_trash = |s: &Step| matches!(s, Step::Trashed(_));
        self.undo.retain(|s| !in_trash(s));
        self.redo.retain(|s| !in_trash(s));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trash_then_undo_restores() {
        use crate::fsops::spawn_op_role;
        use std::sync::atomic::Ordering;
        let root = std::env::temp_dir().join(format!("rivet-undo-trash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("home")).unwrap();
        // A scratch Trash, never the real one. SAFETY: only this test reads XDG_DATA_HOME.
        unsafe { std::env::set_var("XDG_DATA_HOME", root.join("data")) };
        let f = root.join("home/keep.txt");
        std::fs::write(&f, "hi").unwrap();
        let wait = |p: &std::sync::Arc<crate::fsops::Progress>| {
            while !p.finished.load(Ordering::Acquire) {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        let p = spawn_op_role(OpKind::Trash, vec![f.clone()], None, 0);
        wait(&p);
        assert!(!f.exists());
        let Some(Step::Trashed(pairs)) = Step::from_op(OpKind::Trash, p.done.lock().unwrap().clone()) else { panic!("not recorded") };
        assert!(pairs[0].1.starts_with(root.join("data/Trash/files")));
        let back = spawn_op_role(OpKind::Restore, pairs.into_iter().map(|(_, t)| t).collect(), None, 1);
        wait(&back);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "hi");
        unsafe { std::env::remove_var("XDG_DATA_HOME") };
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn redo_is_cleared_by_new_actions() {
        let mut h = History::default();
        h.record(Step::Created("/tmp/a".into()));
        let s = h.pop_undo().unwrap();
        h.record_role(Step::Trashed(vec![("/tmp/a".into(), "/t/a".into())]), 1);
        assert!(h.next_redo().is_some());
        h.record(s);
        assert!(h.next_redo().is_none());
        assert_eq!(h.next_undo().unwrap().describe(), "Create “a”");
    }

    #[test]
    fn copy_move_trash_undo_round_trip() {
        use crate::fsops::{spawn_move_pairs, spawn_op_role};
        use std::sync::atomic::Ordering;
        let wait = |p: &std::sync::Arc<crate::fsops::Progress>| {
            while !p.finished.load(Ordering::Acquire) {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        let root = std::env::temp_dir().join(format!("rivet-undo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (a, b) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("x.txt"), "x").unwrap();

        // Move a/x.txt -> b, then undo by moving it back.
        let p = spawn_op_role(OpKind::Move, vec![a.join("x.txt")], Some(b.clone()), 0);
        wait(&p);
        let Some(Step::Moved(pairs)) = Step::from_op(OpKind::Move, p.done.lock().unwrap().clone()) else { panic!() };
        assert!(b.join("x.txt").exists());
        let back = spawn_move_pairs(pairs.into_iter().map(|(s, d)| (d, s)).collect(), 1);
        wait(&back);
        assert!(a.join("x.txt").exists() && !b.join("x.txt").exists());

        // Copy a/x.txt -> b records the new copy.
        let p = spawn_op_role(OpKind::Copy, vec![a.join("x.txt")], Some(b.clone()), 0);
        wait(&p);
        match Step::from_op(OpKind::Copy, p.done.lock().unwrap().clone()) {
            Some(Step::Copied(c)) => assert_eq!(c, vec![b.join("x.txt")]),
            other => panic!("{other:?}"),
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}
