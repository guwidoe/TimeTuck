//! Original late-installation panic regression; no upstream method credit.
use std::{
    cell::Cell,
    cmp::Ordering,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::{JoinNode, TupleChange};
#[derive(Clone)]
struct Id(usize, Rc<Cell<usize>>);
impl PartialEq for Id {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
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
        let remaining = self.1.get();
        if remaining > 0 {
            self.1.set(remaining - 1);
            assert_ne!(remaining, 1, "injected comparison failure");
        }
        self.0.cmp(&other.0)
    }
}
#[test]
fn every_comparison_failure_preserves_both_join_sides_and_allows_recovery() {
    let count = Rc::new(Cell::new(0));
    let id = |n| Id(n, count.clone());
    let fresh = || {
        let mut join = JoinNode::new(|_: &i64| 0, |_: &i64| 0);
        join.apply_batch(
            (0..6).map(|i| TupleChange::put(id(i), i as i64)),
            (0..6).map(|i| TupleChange::put(id(i), 10 + i as i64)),
        );
        join
    };
    let left = || [TupleChange::put(id(0), 100), TupleChange::put(id(1), 101)];
    let right = || [TupleChange::put(id(0), 200), TupleChange::put(id(1), 201)];
    let mut join = fresh();
    count.set(usize::MAX);
    join.apply_batch(left(), right());
    let comparisons = usize::MAX - count.get();
    assert!(comparisons > 0);
    for failure in 1..=comparisons {
        count.set(0);
        let mut join = fresh();
        count.set(failure);
        assert!(catch_unwind(AssertUnwindSafe(|| join.apply_batch(left(), right()))).is_err());
        count.set(0);
        assert!(!join.is_poisoned());
        let actual: Vec<_> = join
            .matches()
            .map(|((i, j), (a, b))| (i.0, j.0, *a, *b))
            .collect();
        let expected: Vec<_> = (0..6)
            .flat_map(|i| (0..6).map(move |j| (i, j, i as i64, 10 + j as i64)))
            .collect();
        assert_eq!(actual, expected, "failure at comparison {failure}");
        join.apply_batch(left(), right());
        for ((i, j), (a, b)) in join.matches() {
            assert_eq!(
                *a,
                if i.0 < 2 {
                    100 + i.0 as i64
                } else {
                    i.0 as i64
                }
            );
            assert_eq!(
                *b,
                if j.0 < 2 {
                    200 + j.0 as i64
                } else {
                    10 + j.0 as i64
                }
            );
        }
    }
}
