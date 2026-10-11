//! Assignment dirtiness since the last phase-best snapshot. Unlike per-trial
//! winner patches, this union includes committed steps that were not new bests.
use crate::candidate_snapshot::{BasicSnapshot, ListSnapshot, MixedSnapshot};
use crate::{Assignment, ListAssignment, MixedAssignment};

pub(crate) struct DirtySlots {
    size: usize,
    marked: Vec<bool>,
    indices: Vec<usize>,
    all: bool,
}
impl Default for DirtySlots {
    fn default() -> Self {
        Self {
            size: 0,
            marked: Vec::new(),
            indices: Vec::new(),
            all: true,
        }
    }
}
impl DirtySlots {
    fn new(size: usize) -> Self {
        Self {
            size,
            marked: Vec::new(),
            indices: Vec::new(),
            all: false,
        }
    }
    pub(crate) fn record(&mut self, indices: impl Iterator<Item = usize>) {
        if self.all {
            return;
        }
        if self.size <= 32 {
            self.all = true;
            return;
        }
        for index in indices {
            // Empty/whole-replacement-only phases never allocate a bitmap. This allocation
            // precedes publication of the first sparse committed assignment.
            if self.marked.is_empty() {
                self.marked.resize(self.size, false);
            }
            if !self.marked[index] {
                self.marked[index] = true;
                self.indices.push(index);
                if self.indices.len() > self.size / 8 {
                    self.all = true;
                    break;
                }
            }
        }
    }
    pub(crate) fn mark_all(&mut self) {
        self.all = true;
    }
    fn clear(&mut self) {
        for index in self.indices.drain(..) {
            self.marked[index] = false;
        }
        self.all = false;
    }
    fn full(&self) -> bool {
        self.all || self.size <= 32
    }
}
#[derive(Default)]
pub(crate) struct AssignmentChanges {
    pub(crate) basic: DirtySlots,
    pub(crate) lists: DirtySlots,
}
impl AssignmentChanges {
    pub(crate) fn mark_all(&mut self) {
        self.basic.mark_all();
        self.lists.mark_all();
    }
    pub(super) fn clear(&mut self) {
        self.basic.clear();
        self.lists.clear();
    }
}
pub(crate) trait RetainedAssignment: Clone {
    type Patch;
    fn changes(&self) -> AssignmentChanges;
    fn stage(&self, changes: &AssignmentChanges) -> Self::Patch;
    fn install(&mut self, patch: Self::Patch);
}
impl<V: Clone> RetainedAssignment for Assignment<V> {
    type Patch = BasicSnapshot<V>;
    fn changes(&self) -> AssignmentChanges {
        AssignmentChanges {
            basic: DirtySlots::new(self.values().len()),
            ..Default::default()
        }
    }
    fn stage(&self, changes: &AssignmentChanges) -> Self::Patch {
        if changes.basic.full() {
            BasicSnapshot::Full(self.clone())
        } else {
            BasicSnapshot::Sparse(
                changes
                    .basic
                    .indices
                    .iter()
                    .map(|&i| (i, self.values()[i].clone()))
                    .collect(),
            )
        }
    }
    fn install(&mut self, patch: Self::Patch) {
        patch.install(self);
    }
}
impl<V: Clone> RetainedAssignment for ListAssignment<V> {
    type Patch = ListSnapshot<V>;
    fn changes(&self) -> AssignmentChanges {
        AssignmentChanges {
            lists: DirtySlots::new(self.lists().len()),
            ..Default::default()
        }
    }
    fn stage(&self, changes: &AssignmentChanges) -> Self::Patch {
        if changes.lists.full() {
            ListSnapshot::Full(self.clone())
        } else {
            ListSnapshot::Sparse(
                changes
                    .lists
                    .indices
                    .iter()
                    .map(|&i| (i, self.lists()[i].clone()))
                    .collect(),
            )
        }
    }
    fn install(&mut self, patch: Self::Patch) {
        patch.install(self);
    }
}
impl<B: Clone, L: Clone> RetainedAssignment for MixedAssignment<B, L> {
    type Patch = MixedSnapshot<B, L>;
    fn changes(&self) -> AssignmentChanges {
        AssignmentChanges {
            basic: DirtySlots::new(self.basic().values().len()),
            lists: DirtySlots::new(self.lists().lists().len()),
        }
    }
    fn stage(&self, changes: &AssignmentChanges) -> Self::Patch {
        MixedSnapshot {
            basic: self.basic().stage(changes),
            lists: self.lists().stage(changes),
        }
    }
    fn install(&mut self, patch: Self::Patch) {
        self.install_snapshot(patch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retained_scoped_phase::{ScopedChanges, ScopedSnapshot};
    use crate::*;
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    #[derive(Debug)]
    struct Payload {
        id: usize,
        payload: usize,
        clones: Rc<Cell<usize>>,
        fail: Rc<Cell<usize>>,
    }
    impl Clone for Payload {
        fn clone(&self) -> Self {
            self.clones.set(self.clones.get() + 1);
            let at = self.fail.get();
            self.fail.set(at.saturating_sub(1));
            assert_ne!(at, 1, "clone failure");
            Self {
                id: self.id,
                payload: self.payload,
                clones: self.clones.clone(),
                fail: self.fail.clone(),
            }
        }
    }
    impl PartialEq for Payload {
        fn eq(&self, b: &Self) -> bool {
            self.id == b.id
        }
    }
    impl Eq for Payload {}
    fn value(
        id: usize,
        payload: usize,
        clones: &Rc<Cell<usize>>,
        fail: &Rc<Cell<usize>>,
    ) -> Payload {
        Payload {
            id,
            payload,
            clones: clones.clone(),
            fail: fail.clone(),
        }
    }
    fn basic(clones: &Rc<Cell<usize>>, fail: &Rc<Cell<usize>>) -> Assignment<Payload> {
        Assignment {
            values: (0..64).map(|i| Some(value(i, i, clones, fail))).collect(),
        }
    }
    fn lists(clones: &Rc<Cell<usize>>, fail: &Rc<Cell<usize>>) -> ListAssignment<Payload> {
        ListAssignment::new((0..64).map(|i| vec![value(i, i, clones, fail)]).collect())
    }
    fn shape(a: &MixedAssignment<Payload, Payload>) -> (Vec<usize>, Vec<Vec<usize>>) {
        (
            a.basic()
                .values()
                .iter()
                .map(|v| v.as_ref().unwrap().payload)
                .collect(),
            a.lists()
                .lists()
                .iter()
                .map(|r| r.iter().map(|v| v.payload).collect())
                .collect(),
        )
    }
    #[test]
    fn sparse_joint_refresh_stages_all_clones_before_publishing_or_clearing() {
        let clones = Rc::new(Cell::new(0));
        let fail = Rc::new(Cell::new(0));
        for at in 1..=6 {
            let old = MixedAssignment::new(basic(&clones, &fail), lists(&clones, &fail));
            let expected = shape(&old);
            let mut next_basic = basic(&clones, &fail);
            next_basic.values[1] = Some(value(1, 101, &clones, &fail));
            let mut next_lists = lists(&clones, &fail);
            next_lists.lists[2][0].payload = 102;
            next_lists.lists[3][0].payload = 103;
            let next = MixedAssignment::new(next_basic, next_lists);
            let mut changes = ScopedChanges::new(16);
            changes.assignment = next.changes();
            changes.assignment.basic.record([1, 1].into_iter());
            changes.assignment.lists.record([2, 3, 2].into_iter());
            changes.record_batch([0, 2].into_iter());
            let mut snapshot = ScopedSnapshot {
                assignment: old,
                contributions: (0..16).map(|i| value(i, i, &clones, &fail)).collect(),
                score: value(0, 0, &clones, &fail),
            };
            let contributions: Vec<_> =
                (0..16).map(|i| value(i, 100 + i, &clones, &fail)).collect();
            let score = value(0, 999, &clones, &fail);
            fail.set(at);
            assert!(
                catch_unwind(AssertUnwindSafe(|| snapshot.refresh(
                    &next,
                    &contributions,
                    &score,
                    &mut changes
                )))
                .is_err()
            );
            assert_eq!(shape(&snapshot.assignment), expected);
            assert_eq!(snapshot.score.payload, 0);
            assert_eq!(
                snapshot
                    .contributions
                    .iter()
                    .map(|v| v.payload)
                    .collect::<Vec<_>>(),
                (0..16).collect::<Vec<_>>()
            );
            assert_eq!(changes.assignment.basic.indices, [1]);
            assert_eq!(changes.assignment.lists.indices, [2, 3]);
            fail.set(0);
            clones.set(0);
            snapshot.refresh(&next, &contributions, &score, &mut changes);
            assert_eq!(clones.get(), 6);
            assert_eq!(shape(&snapshot.assignment), shape(&next));
            assert!(changes.assignment.basic.indices.is_empty());
            assert!(changes.assignment.lists.indices.is_empty());
        }
    }
    #[test]
    fn committed_union_tracks_nonbest_and_net_zero_edits_without_score_dependencies() {
        let clones = Rc::new(Cell::new(0));
        let fail = Rc::new(Cell::new(0));
        let p = ListProblem::new(
            (0..64).map(|i| value(i, i, &clones, &fail)).collect(),
            (0..64)
                .map(|i| ListEntity::new(vec![value(i, 100 + i, &clones, &fail)]))
                .collect(),
        )
        .unwrap();
        let net = ListConstraintNetwork::<Payload, SimpleScore>::new(64, 64);
        let mut d = ListScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
        let mut d = d.start_retained_tracking();
        let mut best = None;
        d.save_retained_best(&mut best);
        let swap = |a, b| ListMove::Swap {
            left: ListPosition {
                entity: a,
                index: 0,
            },
            right: ListPosition {
                entity: b,
                index: 0,
            },
        };
        d.apply(&swap(0, 1)).unwrap().commit();
        d.apply(&swap(2, 3)).unwrap().commit();
        d.apply(&swap(0, 1)).unwrap().commit(); // Net zero still belongs to the dirty union.
        drop(d.apply(&swap(4, 5)).unwrap()); // Rejected trials never enter the union.
        clones.set(0);
        d.save_retained_best(&mut best);
        assert_eq!(clones.get(), 4);
        let best = best.unwrap();
        assert_eq!(
            best.assignment
                .lists()
                .iter()
                .map(|r| r[0].payload)
                .collect::<Vec<_>>(),
            d.assignment()
                .lists()
                .iter()
                .map(|r| r[0].payload)
                .collect::<Vec<_>>()
        );
        assert_eq!(best.score, SimpleScore::ZERO);
        d.assert_consistent().unwrap();
    }
    #[test]
    fn dense_tiny_and_replacement_boundaries_keep_full_copy_fallbacks() {
        let clones = Rc::new(Cell::new(0));
        let fail = Rc::new(Cell::new(0));
        let a = basic(&clones, &fail);
        let mut changes = a.changes();
        assert!(changes.basic.marked.is_empty());
        changes.basic.record(std::iter::empty());
        assert!(changes.basic.marked.is_empty());
        changes.mark_all();
        assert!(changes.basic.marked.is_empty());
        changes.clear();
        changes.basic.record(0..9);
        clones.set(0);
        let _ = a.stage(&changes);
        assert_eq!(clones.get(), 64);
        changes.clear();
        changes.mark_all();
        clones.set(0);
        let _ = a.stage(&changes);
        assert_eq!(clones.get(), 64);
        let tiny = Assignment {
            values: vec![Some(value(0, 0, &clones, &fail)); 8],
        };
        clones.set(0);
        let _ = tiny.stage(&tiny.changes());
        assert_eq!(clones.get(), 8);
        // Whole replacements retain Eq-equal noncanonical payloads, even without score changes.
        let p = FiniteProblem::new(
            (0..64)
                .map(|i| {
                    Variable::new(vec![value(i, i, &clones, &fail)])
                        .with_initial(value(i, i, &clones, &fail))
                })
                .collect(),
        )
        .unwrap();
        let net = ConstraintNetwork::<Payload, SimpleScore>::new(64);
        let mut d = ScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
        let mut d = d.start_retained_tracking();
        let mut best = None;
        d.save_retained_best(&mut best);
        let mut next = p.initial_assignment();
        next.values[55].as_mut().unwrap().payload = 999;
        d.replace_retained_assignment(next, true).unwrap();
        clones.set(0);
        d.save_retained_best(&mut best);
        assert_eq!(clones.get(), 64);
        assert_eq!(
            best.unwrap().assignment.values()[55]
                .as_ref()
                .unwrap()
                .payload,
            999
        );
    }
}
