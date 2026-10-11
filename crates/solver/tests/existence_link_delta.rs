//! Original link-delta regressions; no upstream source-method credit.
use std::{
    cell::Cell,
    cmp::Ordering,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::{ExistenceMode as Mode, ExistsNode, TupleChange as Change};

#[derive(Debug)]
struct Id {
    rank: usize,
    payload: usize,
    clones: Rc<Cell<usize>>,
    fail: Rc<Cell<Option<(bool, usize)>>>,
}
impl Id {
    fn tick(&self, comparison: bool) {
        if let Some((kind, remaining)) = self.fail.get()
            && kind == comparison
        {
            assert!(remaining > 0, "identity operation failure");
            self.fail.set(Some((kind, remaining - 1)));
        }
    }
}
impl Clone for Id {
    fn clone(&self) -> Self {
        self.tick(false);
        self.clones.set(self.clones.get() + 1);
        Self {
            rank: self.rank,
            payload: self.payload,
            clones: self.clones.clone(),
            fail: self.fail.clone(),
        }
    }
}
impl PartialEq for Id {
    fn eq(&self, other: &Self) -> bool {
        self.rank == other.rank
    }
}
impl Eq for Id {}
impl PartialOrd for Id {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Id {
    fn cmp(&self, other: &Self) -> Ordering {
        self.tick(true);
        self.rank.cmp(&other.rank)
    }
}

#[test]
fn stable_dense_matches_keep_predicate_work_without_recloning_every_reciprocal_id() {
    let clones = Rc::new(Cell::new(0));
    let fail = Rc::new(Cell::new(None));
    let id = |rank| Id {
        rank,
        payload: rank,
        clones: clones.clone(),
        fail: fail.clone(),
    };
    let predicates = Cell::new(0);
    for mode in [Mode::Exists, Mode::NotExists] {
        let mut node = ExistsNode::new(
            mode,
            |_: &usize| (),
            |_: &usize| (),
            |a, b| {
                predicates.set(predicates.get() + 1);
                a != b
            },
        );
        for i in 0..64 {
            node.apply_left(Change::put(id(i), i));
            node.apply_right(Change::put(id(i), i));
        }
        clones.set(0);
        predicates.set(0);
        let left = node.apply_left(Change::put(id(0), 0));
        assert_eq!(left.is_some(), mode == Mode::Exists);
        assert_eq!(predicates.get(), 64);
        assert!(
            clones.get() < 80,
            "stable left identity clones: {}",
            clones.get()
        );
        clones.set(0);
        predicates.set(0);
        assert!(node.apply_right(Change::put(id(0), 0)).is_empty());
        assert_eq!(predicates.get(), 64);
        assert!(
            clones.get() < 80,
            "stable right identity clones: {}",
            clones.get()
        );
    }
}

#[test]
fn surviving_and_new_matches_emit_current_identity_and_actual_row_payload() {
    let clones = Rc::new(Cell::new(0));
    let fail = Rc::new(Cell::new(None));
    let id = |rank, payload| Id {
        rank,
        payload,
        clones: clones.clone(),
        fail: fail.clone(),
    };
    for mode in [Mode::Exists, Mode::NotExists] {
        let mut node = ExistsNode::new(
            mode,
            |_: &(usize, usize)| (),
            |_: &usize| (),
            |l, r| l.0 <= *r,
        );
        node.apply_left(Change::put(id(1, 10), (0, 100)));
        node.apply_left(Change::put(id(2, 20), (2, 200)));
        node.apply_right(Change::put(id(9, 90), 0));
        node.apply_left(Change::put(id(1, 11), (0, 101)));
        node.apply_left(Change::put(id(2, 21), (2, 201)));
        assert!(node.apply_right(Change::put(id(9, 91), 0)).is_empty());
        let entered = node.apply_right(Change::put(id(9, 92), 3));
        assert_eq!(entered.len(), 1);
        assert_eq!((entered[0].id.rank, entered[0].id.payload), (2, 21));
        assert_eq!(entered[0].value, (mode == Mode::Exists).then_some((2, 201)));
        let removed = node.apply_right(Change::retract(id(9, 93)));
        assert_eq!(
            removed
                .iter()
                .map(|c| (c.id.rank, c.id.payload, c.value))
                .collect::<Vec<_>>(),
            vec![
                (1, 11, (mode == Mode::NotExists).then_some((0, 101))),
                (2, 21, (mode == Mode::NotExists).then_some((2, 201)))
            ]
        );
        // set_row removes and reinserts the actual ID, including Ord-equal payload.
        assert!(
            node.rows()
                .all(|(id, _)| id.payload == if id.rank == 1 { 11 } else { 21 })
        );
    }
}

#[test]
fn identity_clone_and_comparison_failures_preserve_both_link_directions() {
    for mode in [Mode::Exists, Mode::NotExists] {
        for right in [false, true] {
            for comparison in [false, true] {
                let mut failures = 0;
                let mut successes = 0;
                for budget in 0..192 {
                    let clones = Rc::new(Cell::new(0));
                    let fail = Rc::new(Cell::new(None));
                    let id = |rank| Id {
                        rank,
                        payload: rank,
                        clones: clones.clone(),
                        fail: fail.clone(),
                    };
                    let mut node =
                        ExistsNode::new(mode, |_: &usize| (), |_: &usize| (), |_, _| true);
                    for i in 0..8 {
                        node.apply_left(Change::put(id(i), 0));
                        node.apply_right(Change::put(id(i), 0));
                    }
                    let before = node.rows().map(|(i, r)| (i.rank, *r)).collect::<Vec<_>>();
                    fail.set(Some((comparison, budget)));
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        if right {
                            node.apply_right(Change::put(id(0), 1));
                        } else {
                            node.apply_left(Change::put(id(0), 1));
                        }
                    }));
                    fail.set(None);
                    if result.is_err() {
                        failures += 1;
                        assert_eq!(
                            node.rows().map(|(i, r)| (i.rank, *r)).collect::<Vec<_>>(),
                            before
                        );
                    } else {
                        successes += 1;
                    }
                    for i in 0..8 {
                        node.apply_right(Change::retract(id(i)));
                    }
                    assert_eq!(
                        node.rows().count(),
                        if mode == Mode::Exists { 0 } else { 8 }
                    );
                    for i in 0..8 {
                        node.apply_left(Change::retract(id(i)));
                    }
                    assert_eq!(node.rows().count(), 0);
                }
                assert!(
                    failures > 0 && successes > 0,
                    "{mode:?} right={right} comparison={comparison}"
                );
            }
        }
    }
}
