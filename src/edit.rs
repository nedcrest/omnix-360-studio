//! Undo: a sheet and the states it was in, as Omnix's `Edit` keeps a
//! project's. Each op is a step; a run of them (a segment's edge dragged)
//! is one, between `begin` and `commit`; a step that changed nothing is
//! no step at all.

use crate::{Doc, Op, OpError, Outcome};

/// How many steps back undo goes.
const HISTORY: usize = 200;

/// A sheet being changed, with its undo and redo.
#[derive(Debug, Clone)]
pub struct Edit {
    doc: Doc,
    undo: Vec<Doc>,
    redo: Vec<Doc>,
    /// The state when a run of changes began, while it's under way.
    open: Option<Doc>,
}

impl Edit {
    pub fn new(doc: Doc) -> Edit {
        Edit { doc, undo: Vec::new(), redo: Vec::new(), open: None }
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    /// Applies `op`: a step of its own, or part of the run under way. A
    /// refusal changes nothing and leaves no step.
    pub fn apply(&mut self, op: &Op) -> Result<Outcome, OpError> {
        let before = self.doc.clone();
        let outcome = self.doc.apply(op)?;
        if self.open.is_none() {
            self.remember(before);
        }
        Ok(outcome)
    }

    /// Starts a run of changes to be one step; while one is under way,
    /// this does nothing.
    pub fn begin(&mut self) {
        if self.open.is_none() {
            self.open = Some(self.doc.clone());
        }
    }

    /// Ends the run: one step, or none if it changed nothing.
    pub fn commit(&mut self) {
        if let Some(before) = self.open.take() {
            self.remember(before);
        }
    }

    /// Back one step; false with nothing to undo.
    pub fn undo(&mut self) -> bool {
        self.commit();
        let Some(before) = self.undo.pop() else { return false };
        self.redo.push(std::mem::replace(&mut self.doc, before));
        true
    }

    /// Forward one step undone; false with nothing to redo.
    pub fn redo(&mut self) -> bool {
        self.commit();
        let Some(after) = self.redo.pop() else { return false };
        self.undo.push(std::mem::replace(&mut self.doc, after));
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.open.as_ref().is_some_and(|o| *o != self.doc)
    }

    /// A step back to `before`, if anything changed since it.
    fn remember(&mut self, before: Doc) {
        if before == self.doc {
            return;
        }
        self.undo.push(before);
        if self.undo.len() > HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipOp, MarkOp};

    const CLIP: &str = "2030-05-01/a.mp4";

    fn edit() -> Edit {
        Edit::new(Doc::parse("{}").unwrap())
    }

    fn rate(r: u8) -> Op {
        Op::Clip(ClipOp::Rate { clip: CLIP.into(), rating: Some(r) })
    }

    #[test]
    fn each_op_is_a_step_and_undo_and_redo_walk_them() {
        let mut e = edit();
        let empty = e.doc().text();
        e.apply(&rate(1)).unwrap();
        let one = e.doc().text();
        e.apply(&rate(2)).unwrap();
        assert!(e.undo());
        assert_eq!(e.doc().text(), one);
        assert!(e.undo());
        assert_eq!(e.doc().text(), empty);
        assert!(!e.undo());
        assert!(e.redo() && e.redo());
        assert!(!e.redo());
    }

    #[test]
    fn a_run_is_one_step_and_nothing_changed_is_none() {
        let mut e = edit();
        let id = e.apply(&Op::Mark(MarkOp::Add { clip: CLIP.into(), t: 1.0, end: Some(3.0), flag: false, role: None, note: String::new() })).unwrap().made.unwrap();
        let added = e.doc().text();
        // A drag: many moves, one step.
        e.begin();
        for end in [3.5, 4.0, 4.5] {
            e.apply(&Op::Mark(MarkOp::Move { mark: id.clone(), t: 1.0, end: Some(end) })).unwrap();
        }
        e.commit();
        assert!(e.undo());
        assert_eq!(e.doc().text(), added);
        // The same rating twice: the second is no step.
        e.apply(&rate(2)).unwrap();
        e.apply(&rate(2)).unwrap();
        assert!(e.undo());
        assert_eq!(e.doc().text(), added);
        // A run that changed nothing leaves no step.
        e.begin();
        e.commit();
        assert!(e.undo());
        assert!(!e.undo());
    }

    #[test]
    fn a_refusal_leaves_no_step_and_a_new_change_ends_redo() {
        let mut e = edit();
        e.apply(&rate(1)).unwrap();
        assert!(e.apply(&rate(9)).is_err());
        assert!(e.undo());
        assert!(!e.undo());
        assert!(e.redo());
        e.undo();
        e.apply(&rate(0)).unwrap();
        assert!(!e.redo());
    }
}
