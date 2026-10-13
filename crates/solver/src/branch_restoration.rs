//! Optional recorded-edge restoration for an externally scheduled exhaustive frontier.
use crate::{RecordedMove, ReplayDirection, StaleMoveRecord};
use std::{error::Error, fmt};
/// Basic independent slots may retain the common ancestor. List insertion paths
/// may depend on the whole prefix; replay those from the root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchRestoreMode {
    CommonAncestor,
    RootReplay,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BranchRestoreError<E> {
    UnknownNode { node: usize },
    ParentState,
    Stale(StaleMoveRecord),
    Execute(E),
    UnexpectedResult,
}
impl<E: fmt::Debug> fmt::Display for BranchRestoreError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "branch restoration: {self:?}")
    }
}
impl<E: fmt::Debug> Error for BranchRestoreError<E> {}
#[derive(Debug)]
struct Edge<A, M> {
    parent: usize,
    record: RecordedMove<A, M>,
}
/// Arena of actual recorded moves rooted in one assignment. Nodes are stable
/// indices within this arena. Child insertion validates the complete parent
/// preimage. Restoration executes undo edges bottom-up, then forward edges top-down;
/// it never replaces the assignment wholesale. Successful edges advance the cursor
/// before observer calls. Errors retain completed progress for retry; execution
/// callbacks must leave state unchanged on error, as native model transactions do.
/// Existing exhaustive enumeration keeps its current snapshot paths unchanged.
#[derive(Debug)]
pub struct BranchRestorer<A, M> {
    root: A,
    edges: Vec<Edge<A, M>>,
    current: usize,
}
impl<A: Eq, M> BranchRestorer<A, M> {
    pub fn new(root: A) -> Self {
        Self {
            root,
            edges: vec![],
            current: 0,
        }
    }
    pub fn current_node(&self) -> usize {
        self.current
    }
    pub fn assignment_at(
        &self,
        node: usize,
    ) -> Result<&A, BranchRestoreError<std::convert::Infallible>> {
        if node == 0 {
            Ok(&self.root)
        } else {
            self.edges
                .get(node - 1)
                .map(|e| e.record.after())
                .ok_or(BranchRestoreError::UnknownNode { node })
        }
    }
    pub fn add_child(
        &mut self,
        parent: usize,
        record: RecordedMove<A, M>,
    ) -> Result<usize, BranchRestoreError<std::convert::Infallible>> {
        if self.assignment_at(parent)? != record.before() {
            return Err(BranchRestoreError::ParentState);
        }
        self.edges.push(Edge { parent, record });
        Ok(self.edges.len())
    }
    fn path(&self, mut node: usize) -> Vec<usize> {
        let mut path = vec![];
        while node != 0 {
            path.push(node);
            node = self.edges[node - 1].parent;
        }
        path
    }
    /// The working assignment must equal the current arena node. Each callback
    /// receives the real guarded move and commits it through the caller's model.
    /// Observer events identify successfully completed edge executions only.
    pub fn restore<E>(
        &mut self,
        target: usize,
        mode: BranchRestoreMode,
        working: &mut A,
        mut execute: impl FnMut(&mut A, &M) -> Result<(), E>,
        mut observe: impl FnMut(usize, ReplayDirection),
    ) -> Result<(), BranchRestoreError<E>> {
        if target > self.edges.len() {
            return Err(BranchRestoreError::UnknownNode { node: target });
        }
        if working != self.assignment_at(self.current).expect("valid current") {
            return Err(BranchRestoreError::ParentState);
        }
        let old = self.path(self.current);
        let mut new = self.path(target);
        let common = if mode == BranchRestoreMode::RootReplay {
            0
        } else {
            old.iter()
                .rev()
                .zip(new.iter().rev())
                .take_while(|(old, new)| old == new)
                .map(|(&node, _)| node)
                .last()
                .unwrap_or(0)
        };
        for node in old.into_iter().take_while(|n| *n != common) {
            let edge = &self.edges[node - 1];
            let movement = edge
                .record
                .undo(working)
                .map_err(BranchRestoreError::Stale)?;
            execute(working, movement).map_err(BranchRestoreError::Execute)?;
            if working != edge.record.before() {
                return Err(BranchRestoreError::UnexpectedResult);
            }
            self.current = edge.parent;
            observe(node, ReplayDirection::Undo);
        }
        new.truncate(new.iter().position(|n| *n == common).unwrap_or(new.len()));
        for node in new.into_iter().rev() {
            let edge = &self.edges[node - 1];
            let movement = edge
                .record
                .forward(working)
                .map_err(BranchRestoreError::Stale)?;
            execute(working, movement).map_err(BranchRestoreError::Execute)?;
            if working != edge.record.after() {
                return Err(BranchRestoreError::UnexpectedResult);
            }
            self.current = node;
            observe(node, ReplayDirection::Forward);
        }
        Ok(())
    }
}
