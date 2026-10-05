//! Original regressions for stable-key join replacement and reduced propagation.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[derive(Debug)]
struct Key(i32, Rc<Cell<usize>>);
impl Clone for Key {
    fn clone(&self) -> Self {
        self.1.set(self.1.get() + 1);
        Self(self.0, self.1.clone())
    }
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}
#[test]
fn stable_key_replacements_keep_bucket_membership_on_both_sides() {
    let clones = Rc::new(Cell::new(0));
    let mut join = JoinNode::new(
        |r: &(i32, i64)| Key(r.0, clones.clone()),
        |r: &(i32, i64)| Key(r.0, clones.clone()),
    );
    for i in 0..3 {
        join.apply_left(TupleChange::put(i, (7, i as i64)));
        join.apply_right(TupleChange::put(i, (7, 10 + i as i64)));
    }
    // One unrelated bucket must remain unaffected.
    join.apply_left(TupleChange::put(10, (8, 100)));
    join.apply_right(TupleChange::put(10, (8, 200)));
    clones.set(0);
    assert_eq!(
        join.apply_left(TupleChange::put(1, (7, 90))),
        (0..3)
            .map(|i| TupleChange::put((1, i), ((7, 90), (7, 10 + i as i64))))
            .collect::<Vec<_>>()
    );
    assert_eq!(clones.get(), 0, "stable membership requires no key clones");
    assert_eq!(
        join.apply_right(TupleChange::put(2, (7, 80))),
        (0..3)
            .map(|i| TupleChange::put((i, 2), ((7, if i == 1 { 90 } else { i as i64 }), (7, 80))))
            .collect::<Vec<_>>()
    );
    assert_eq!(clones.get(), 0);
    let changed = join.apply_left(TupleChange::put(1, (8, 60)));
    assert_eq!(
        changed,
        vec![
            TupleChange::retract((1, 0)),
            TupleChange::retract((1, 1)),
            TupleChange::retract((1, 2)),
            TupleChange::put((1, 10), ((8, 60), (8, 200)))
        ]
    );
    assert!(clones.get() > 0);
    assert_eq!(join.matches().count(), 8);
    assert_eq!(
        join.apply_right(TupleChange::retract(2)),
        vec![TupleChange::retract((0, 2)), TupleChange::retract((2, 2))]
    );
    assert_eq!(join.matches().count(), 6);
}
fn apply<K: Ord, V>(
    table: &mut BTreeMap<K, V>,
    changes: impl IntoIterator<Item = TupleChange<K, V>>,
) {
    for c in changes {
        if let Some(v) = c.value {
            table.insert(c.id, v);
        } else {
            table.remove(&c.id);
        }
    }
}
#[test]
fn self_join_payload_updates_match_independent_pairs_in_either_side_order() {
    let mut join = JoinNode::new(|r: &(i32, i64)| r.0, |r: &(i32, i64)| r.0);
    let mut rows = BTreeMap::new();
    let mut matches = BTreeMap::new();
    let mut seed = 20261025u64;
    for step in 0..2000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = (seed >> 32) % 24;
        let value = if step % 9 == 0 {
            None
        } else {
            Some(((id % 3) as i32, step as i64 - 1000))
        };
        if let Some(v) = value {
            rows.insert(id, v);
        } else {
            rows.remove(&id);
        }
        let change = TupleChange { id, value };
        if step % 2 == 0 {
            apply(&mut matches, join.apply_left(change.clone()));
            apply(&mut matches, join.apply_right(change));
        } else {
            apply(&mut matches, join.apply_right(change.clone()));
            apply(&mut matches, join.apply_left(change));
        }
        let expected = rows
            .iter()
            .flat_map(|(&i, &a)| {
                rows.iter()
                    .filter(move |(_, b)| a.0 == b.0)
                    .map(move |(&j, &b)| ((i, j), (a, b)))
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(matches, expected, "step {step}");
        assert_eq!(
            join.matches()
                .map(|((i, j), (a, b))| ((*i, *j), (*a, *b)))
                .collect::<BTreeMap<_, _>>(),
            expected
        );
    }
}
#[test]
fn replacing_join_rows_preserves_group_matches_callbacks_and_transaction_rollback() {
    let calls = Cell::new(0);
    let fail = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let left = b.source::<usize, (usize, i64)>();
    let right = b.source::<usize, (usize, i64)>();
    let joined = b
        .join_equal(left.port(), right.port(), |r| r.0, |r| r.0)
        .unwrap();
    let filtered = b.filter(&joined, |(a, b)| a.1 + b.1 >= 0).unwrap();
    let groups = b
        .group_collect(&filtered, |(a, b)| (a.0, a.1 + b.1))
        .unwrap();
    let terminal = b
        .score("sum", &groups, |(_, s)| {
            calls.set(calls.get() + 1);
            assert_ne!(fail.get(), 2, "terminal panic");
            SimpleScore::new(s.sum).with_structural(if fail.get() == 1 { -1 } else { 0 })
        })
        .unwrap();
    let mut graph = b.build();
    let mut t = graph.transaction();
    t.apply(&left, TupleChange::put(0, (7, 1))).unwrap();
    for i in 0..3 {
        t.apply(&right, TupleChange::put(i, (7, 10 + i as i64)))
            .unwrap();
    }
    t.commit().unwrap();
    let saved = graph.constraint_matches(&terminal).unwrap().clone();
    assert_eq!(graph.score(), SimpleScore::new(36));
    calls.set(0);
    {
        let mut t = graph.transaction();
        t.apply(&left, TupleChange::put(0, (7, 5))).unwrap();
        assert_eq!(t.score().unwrap(), SimpleScore::new(48));
        assert_eq!(calls.get(), 3, "one final group update per surviving pair");
        assert_eq!(
            t.graph().unwrap().constraint_matches(&terminal).unwrap()[&7]
                .0
                .1
                .count,
            3
        );
    }
    assert_eq!(graph.constraint_matches(&terminal).unwrap(), &saved);
    assert_eq!(calls.get(), 3);
    for failure in [1, 2] {
        fail.set(failure);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut t = graph.transaction();
            t.apply(&left, TupleChange::put(0, (7, 9)))?;
            t.commit()
        }));
        if failure == 1 {
            assert!(result.unwrap().is_err());
        } else {
            assert!(result.is_err());
        }
        fail.set(0);
        assert_eq!(graph.constraint_matches(&terminal).unwrap(), &saved);
        graph.assert_consistent().unwrap();
    }
    // Replacements must still retract filter-rejected pairs, then reintroduce them.
    let mut t = graph.transaction();
    t.apply(&left, TupleChange::put(0, (7, -12))).unwrap();
    t.commit().unwrap();
    assert_eq!(
        graph.constraint_matches(&terminal).unwrap()[&7].0.1.count,
        1
    );
    assert_eq!(graph.score(), SimpleScore::ZERO);
    let mut t = graph.transaction();
    t.apply(&left, TupleChange::put(0, (7, 5))).unwrap();
    t.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::new(48));
    assert_eq!(saved[&7].1, SimpleScore::new(36));
    graph.assert_consistent().unwrap();
}

#[test]
fn terminal_snapshots_keep_zero_and_wrapping_contributions_with_current_payloads() {
    let mut b =
        StreamGraphBuilder::<SimpleScore>::new().with_arithmetic(ScoringArithmetic::Wrapping);
    let source = b.source::<usize, (String, i64)>();
    let terminal = b
        .score("weights", source.port(), |r| SimpleScore::new(r.1))
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&source, TupleChange::put(0, ("old".into(), 0)))
        .unwrap();
    t.apply(&source, TupleChange::put(1, ("max".into(), i64::MAX)))
        .unwrap();
    t.apply(&source, TupleChange::put(2, ("one".into(), 1)))
        .unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(i64::MIN));
    let old = g.constraint_matches(&terminal).unwrap().clone();
    {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::put(0, ("trial".into(), 0)))
            .unwrap();
        t.apply(&source, TupleChange::retract(1)).unwrap();
        assert_eq!(t.score().unwrap(), SimpleScore::new(1));
        t.assert_consistent().unwrap();
    }
    assert_eq!(g.constraint_matches(&terminal).unwrap(), &old);
    let mut t = g.transaction();
    t.apply(&source, TupleChange::put(0, ("new".into(), 0)))
        .unwrap();
    t.commit().unwrap();
    assert_eq!(g.constraint_matches(&terminal).unwrap()[&0].0.0, "new");
    assert_eq!(old[&0].0.0, "old");
    assert_eq!(g.score(), SimpleScore::new(i64::MIN));
    g.assert_consistent().unwrap();
}
