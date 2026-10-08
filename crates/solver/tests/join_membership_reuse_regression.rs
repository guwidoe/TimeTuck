//! Original candidate-work regressions; no upstream method credits.
use std::{cell::Cell, cmp::Ordering, rc::Rc};
use timetuck::{JoinComparison, JoinNode, TupleChange};
#[derive(Debug)]
struct Id {
    value: usize,
    clones: Rc<Cell<usize>>,
}
impl Clone for Id {
    fn clone(&self) -> Self {
        if self.value < 64 {
            self.clones.set(self.clones.get() + 1);
        }
        Self {
            value: self.value,
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Id {
    fn eq(&self, r: &Self) -> bool {
        self.value == r.value
    }
}
impl Eq for Id {}
impl PartialOrd for Id {
    fn partial_cmp(&self, r: &Self) -> Option<Ordering> {
        Some(self.cmp(r))
    }
}
impl Ord for Id {
    fn cmp(&self, r: &Self) -> Ordering {
        self.value.cmp(&r.value)
    }
}
#[test]
fn unchanged_membership_clones_each_candidate_identity_once() {
    for comparison in [
        JoinComparison::Equal,
        JoinComparison::LessThan,
        JoinComparison::LessThanOrEqual,
        JoinComparison::GreaterThan,
        JoinComparison::GreaterThanOrEqual,
    ] {
        let clones = Rc::new(Cell::new(0));
        let mut node = JoinNode::comparing(comparison, |r: &(i32, i32)| r.0, |r: &(i32, i32)| r.0);
        node.apply_batch(
            [TupleChange::put(0usize, (2, 10))],
            (0..64).map(|i| {
                TupleChange::put(
                    Id {
                        value: i,
                        clones: clones.clone(),
                    },
                    ((i % 4) as i32, i as i32),
                )
            }),
        );
        clones.set(0);
        let result = node.apply_batch([TupleChange::put(0, (2, 20))], []);
        let expected: Vec<_> = (0..64)
            .filter(|i| comparison.matches(&2, &((i % 4) as i32)))
            .collect();
        assert_eq!(clones.get(), expected.len());
        assert_eq!(
            result.iter().map(|r| r.id.1.value).collect::<Vec<_>>(),
            expected
        );
        for change in result {
            let id = change.id.1.value;
            assert_eq!(change.value, Some(((2, 20), ((id % 4) as i32, id as i32))));
        }
    }
}
#[test]
fn unrelated_bucket_changes_keep_reuse_but_new_members_are_published() {
    let clones = Rc::new(Cell::new(0));
    let id = |value| Id {
        value,
        clones: clones.clone(),
    };
    let mut node = JoinNode::new(|r: &(i32, i32)| r.0, |r: &(i32, i32)| r.0);
    node.apply_batch(
        [TupleChange::put(0usize, (0, 10))],
        (0..65).map(|i| TupleChange::put(id(i), (i32::from(i == 64), i as i32))),
    );
    clones.set(0);
    let result = node.apply_batch(
        [TupleChange::put(0, (0, 20))],
        [TupleChange::put(id(64), (2, 64))],
    );
    assert_eq!(clones.get(), 64);
    assert_eq!(
        result.iter().map(|r| r.id.1.value).collect::<Vec<_>>(),
        (0..64).collect::<Vec<_>>()
    );
    let result = node.apply_batch(
        [TupleChange::put(0, (0, 30))],
        [TupleChange::put(id(64), (0, 64))],
    );
    assert_eq!(result.len(), 65);
    for (i, event) in result.iter().enumerate() {
        assert_eq!(event.id.1.value, i);
        assert_eq!(event.value, Some(((0, 30), (0, i as i32))));
    }
    let result = node.apply_batch(
        [TupleChange::put(0, (0, 40))],
        [TupleChange::retract(id(5))],
    );
    assert_eq!(result.len(), 65);
    assert_eq!(result[5].value, None);
    assert_eq!(node.matches().count(), 64);
}

#[test]
fn batch_buffers_keep_first_id_representative_and_final_payload() {
    #[derive(Clone, Debug)]
    struct Representative(usize, &'static str);
    impl PartialEq for Representative {
        fn eq(&self, r: &Self) -> bool {
            self.0 == r.0
        }
    }
    impl Eq for Representative {}
    impl PartialOrd for Representative {
        fn partial_cmp(&self, r: &Self) -> Option<Ordering> {
            Some(self.cmp(r))
        }
    }
    impl Ord for Representative {
        fn cmp(&self, r: &Self) -> Ordering {
            self.0.cmp(&r.0)
        }
    }
    let mut node = JoinNode::new(|_: &i32| 0, |_: &i32| 0);
    let result = node.apply_batch(
        [
            TupleChange::put(Representative(0, "first"), 1),
            TupleChange::put(Representative(0, "second"), 2),
            TupleChange::retract(Representative(0, "third")),
            TupleChange::put(Representative(0, "last"), 7),
        ],
        [TupleChange::put(0usize, 3)],
    );
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id.0.1, "first");
    assert_eq!(result[0].value, Some((7, 3)));
    let result = node.apply_batch(
        [
            TupleChange::put(Representative(0, "update"), 9),
            TupleChange::put(Representative(0, "ignored"), 11),
        ],
        [TupleChange::put(0, 4), TupleChange::put(0, 5)],
    );
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id.0.1, "update");
    assert_eq!(result[0].value, Some((11, 5)));
    let result = node.apply_batch(
        [
            TupleChange::put(Representative(0, "remove"), 12),
            TupleChange::retract(Representative(0, "last")),
        ],
        [],
    );
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id.0.1, "remove");
    assert_eq!(result[0].value, None);
    assert_eq!(node.matches().count(), 0);
}
