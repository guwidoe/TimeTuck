//! Recycle one empty carrier after a node has consumed its journal value.
//! Full snapshots never enter this cache. An idle box owns no score/key/payload.
use std::any::Any;

type UnpackedCheckpoint<T> = Result<(T, Box<Option<T>>), Box<dyn Any>>;

pub(crate) struct CheckpointBox<T> {
    spare: Option<Box<Option<T>>>,
}
impl<T: 'static> CheckpointBox<T> {
    pub(crate) fn new() -> Self {
        Self { spare: None }
    }
    pub(crate) fn begin(&mut self, value: T) -> Box<dyn Any> {
        let mut saved = self.spare.take().unwrap_or_else(|| Box::new(None));
        *saved = Some(value);
        saved
    }
    pub(crate) fn get(saved: &dyn Any) -> Option<&T> {
        saved
            .downcast_ref::<Option<T>>()
            .map(|saved| saved.as_ref().expect("active journal checkpoint"))
    }
    pub(crate) fn unpack(saved: Box<dyn Any>) -> UnpackedCheckpoint<T> {
        saved.downcast::<Option<T>>().map(|mut saved| {
            let value = saved.take().expect("active journal checkpoint");
            (value, saved)
        })
    }
    pub(crate) fn recycle(&mut self, saved: Box<Option<T>>) {
        assert!(saved.is_none(), "recycle only an empty journal carrier");
        self.spare = Some(saved);
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::Score;
    use std::{
        cell::{Cell, RefCell},
        cmp::Ordering,
        ops::{Add, Sub},
        panic::{AssertUnwindSafe, catch_unwind},
    };

    #[derive(Debug, Default)]
    struct Probe {
        events: RefCell<Vec<(&'static str, i64)>>,
        fail_clone: Cell<bool>,
    }
    #[derive(Debug)]
    struct TracedScore {
        value: i64,
        probe: Option<Rc<Probe>>,
    }
    impl Clone for TracedScore {
        fn clone(&self) -> Self {
            if let Some(probe) = &self.probe {
                probe.events.borrow_mut().push(("clone", self.value));
                assert!(!probe.fail_clone.get(), "score clone");
            }
            Self {
                value: self.value,
                probe: self.probe.clone(),
            }
        }
    }
    impl Drop for TracedScore {
        fn drop(&mut self) {
            if let Some(probe) = &self.probe {
                probe.events.borrow_mut().push(("drop", self.value));
            }
        }
    }
    impl PartialEq for TracedScore {
        fn eq(&self, other: &Self) -> bool {
            self.value == other.value
        }
    }
    impl Eq for TracedScore {}
    impl PartialOrd for TracedScore {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for TracedScore {
        fn cmp(&self, other: &Self) -> Ordering {
            self.value.cmp(&other.value)
        }
    }
    impl Score for TracedScore {
        type Level = i64;
        const HARD_LEVEL_COUNT: usize = 0;
        fn ordinary_levels(&self) -> impl Iterator<Item = i64> {
            std::iter::once(self.value)
        }
        fn structural(&self) -> i64 {
            0
        }
        fn hard_cmp(&self, _: &Self) -> Ordering {
            Ordering::Equal
        }
        fn is_feasible(&self) -> bool {
            true
        }
    }
    impl Add for TracedScore {
        type Output = Self;
        fn add(self, rhs: Self) -> Self {
            Self {
                value: self.value + rhs.value,
                probe: self.probe.clone().or_else(|| rhs.probe.clone()),
            }
        }
    }
    impl Sub for TracedScore {
        type Output = Self;
        fn sub(self, rhs: Self) -> Self {
            Self {
                value: self.value - rhs.value,
                probe: self.probe.clone().or_else(|| rhs.probe.clone()),
            }
        }
    }
    impl AdditiveScore for TracedScore {
        const ZERO: Self = Self {
            value: 0,
            probe: None,
        };
        fn with_structural(self, structural: i64) -> Self {
            assert_eq!(structural, 0);
            self
        }
    }
    fn address(saved: &dyn Any) -> *const TerminalJournal<TracedScore> {
        CheckpointBox::<TerminalJournal<TracedScore>>::get(saved).unwrap()
    }

    #[test]
    fn all_terminal_kinds_release_cloned_scores_before_recycling_and_clone_before_opening_frames() {
        for kind in 0..3 {
            let probe = Rc::new(Probe::default());
            let mut builder = StreamGraphBuilder::<TracedScore>::new();
            let source = builder.source::<usize, i64>();
            let evaluate = {
                let probe = probe.clone();
                move |value: &i64| TracedScore {
                    value: *value,
                    probe: Some(probe.clone()),
                }
            };
            let constraint = match kind {
                0 => builder.score("ordinary", source.port(), evaluate),
                1 => builder.score_fixed("fixed", source.port(), [0], evaluate),
                _ => builder.score_only("only", source.port(), evaluate),
            }
            .unwrap();
            let mut graph = builder.build();
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(0, 7)).unwrap();
            tx.commit().unwrap();
            let node = &mut graph.nodes[constraint.0.node];
            probe.events.borrow_mut().clear();
            let committed = node.begin_checkpoint();
            let first_address = address(committed.as_ref());
            node.commit_checkpoint(committed);
            assert_eq!(*probe.events.borrow(), [("clone", 7), ("drop", 7)]);

            probe.events.borrow_mut().clear();
            probe.fail_clone.set(true);
            assert!(catch_unwind(AssertUnwindSafe(|| node.begin_checkpoint())).is_err());
            assert_eq!(*probe.events.borrow(), [("clone", 7)]);
            probe.fail_clone.set(false);
            probe.events.borrow_mut().clear();
            let outer = node.begin_checkpoint();
            assert_eq!(first_address, address(outer.as_ref()));
            assert!(node.scores_match(outer.as_ref()));
            let inner = node.begin_checkpoint();
            assert_ne!(address(outer.as_ref()), address(inner.as_ref()));
            node.commit_checkpoint(inner);
            node.restore(outer);
            assert_eq!(
                *probe.events.borrow(),
                [("clone", 7), ("clone", 7), ("drop", 7), ("drop", 7)]
            );

            // Full saved candidates remain distinct, usable inside a live frame.
            probe.events.borrow_mut().clear();
            let full = node.checkpoint();
            let journal = node.begin_checkpoint();
            assert_eq!(first_address, address(journal.as_ref()));
            node.restore(full);
            assert!(node.scores_match(journal.as_ref()));
            node.restore(journal);
            assert_eq!(
                *probe.events.borrow(),
                [("clone", 7), ("clone", 7), ("drop", 7), ("drop", 7)]
            );
            probe.events.borrow_mut().clear();
            let final_round = node.begin_checkpoint();
            assert_eq!(first_address, address(final_round.as_ref()));
            node.commit_checkpoint(final_round);
            assert_eq!(*probe.events.borrow(), [("clone", 7), ("drop", 7)]);

            // Actual row writes must release both the displaced row contribution
            // and cloned total at commit, or both candidate values at rollback.
            let arithmetic = NodeArithmetic {
                impact: crate::ScoringArithmetic::Checked,
                totals: crate::ScoringArithmetic::Checked,
            };
            let committed = node.begin_checkpoint();
            let update = Event::new(TupleChange::put(0usize, 8_i64));
            node.apply(false, &update, arithmetic).unwrap();
            probe.events.borrow_mut().clear();
            node.commit_checkpoint(committed);
            assert_eq!(*probe.events.borrow(), [("drop", 7), ("drop", 7)]);
            let rolled_back = node.begin_checkpoint();
            let update = Event::new(TupleChange::put(0usize, 9_i64));
            node.apply(false, &update, arithmetic).unwrap();
            assert!(!node.scores_match(rolled_back.as_ref()));
            probe.events.borrow_mut().clear();
            node.restore(rolled_back);
            assert_eq!(*probe.events.borrow(), [("drop", 9), ("drop", 9)]);
        }
    }
}
