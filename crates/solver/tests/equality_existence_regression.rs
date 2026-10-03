//! Original regressions; no upstream coverage credit.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::{
    ExistenceMode as Mode, ExistsEqualNode, SimpleScore, StreamGraphBuilder, TupleChange as Change,
};

type Row = (u32, i64);
fn predicate(_: &Row, _: &Row) -> bool {
    true
}
fn random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed >> 32
}
#[test]
fn indexed_edits_match_independent_nested_enumeration() {
    for mode in [Mode::Exists, Mode::NotExists] {
        let mut node = ExistsEqualNode::<usize, usize, Row, Row, u32>::new(mode, |a| a.0, |b| b.0);
        let mut left = BTreeMap::new();
        let mut right = BTreeMap::new();
        let mut emitted = BTreeMap::new();
        let mut seed = 20261018;
        for step in 0..3000 {
            let id = (random(&mut seed) % 32) as usize;
            let value = (!random(&mut seed).is_multiple_of(4)).then(|| {
                (
                    (random(&mut seed) % 7) as u32,
                    (random(&mut seed) % 15) as i64 - 7,
                )
            });
            let changes = if step % 2 == 0 {
                if let Some(row) = value {
                    left.insert(id, row);
                } else {
                    left.remove(&id);
                }
                node.apply_left(Change { id, value })
                    .into_iter()
                    .collect::<Vec<_>>()
            } else {
                if let Some(row) = value {
                    right.insert(id, row);
                } else {
                    right.remove(&id);
                }
                node.apply_right(Change { id, value })
            };
            for change in changes {
                if let Some(row) = change.value {
                    emitted.insert(change.id, row);
                } else {
                    emitted.remove(&change.id);
                }
            }
            let expected: BTreeMap<_, _> = left
                .iter()
                .filter(|(_, a)| {
                    right.values().any(|b| a.0 == b.0 && predicate(a, b)) == (mode == Mode::Exists)
                })
                .map(|(&i, &r)| (i, r))
                .collect();
            assert_eq!(emitted, expected, "mode {mode:?}, step {step}");
            assert_eq!(
                node.rows()
                    .map(|(&i, &r)| (i, r))
                    .collect::<BTreeMap<_, _>>(),
                expected
            );
        }
    }
}
#[test]
fn self_existence_graph_commits_and_rollbacks_match_full_scores() {
    for mode in [Mode::Exists, Mode::NotExists] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, u32, i64)>();
        let exists = b
            .existence_equal(source.port(), source.port(), mode, |a| a.1, |b| b.1)
            .unwrap();
        let terminal = b
            .score("exists", &exists, |a| SimpleScore::new(a.2))
            .unwrap();
        let mut graph = b.build();
        let mut rows = BTreeMap::new();
        let mut seed = 20261019;
        for step in 0..1000 {
            let id = (random(&mut seed) % 20) as usize;
            let value = (!random(&mut seed).is_multiple_of(4)).then(|| {
                (
                    id,
                    (random(&mut seed) % 4) as u32,
                    (random(&mut seed) % 13) as i64 - 6,
                )
            });
            let mut candidate = rows.clone();
            if let Some(row) = value {
                candidate.insert(id, row);
            } else {
                candidate.remove(&id);
            }
            let mut tx = graph.transaction();
            tx.apply(&source, Change { id, value }).unwrap();
            let expected: BTreeMap<_, _> = candidate
                .iter()
                .filter(|(_, a)| candidate.values().any(|b| a.1 == b.1) == (mode == Mode::Exists))
                .map(|(&i, &r)| (i, (r, SimpleScore::new(r.2))))
                .collect();
            assert_eq!(
                tx.graph().unwrap().constraint_matches(&terminal).unwrap(),
                &expected
            );
            assert_eq!(
                tx.score().unwrap(),
                SimpleScore::new(expected.values().map(|(r, _)| r.2).sum())
            );
            if step % 3 == 0 {
                drop(tx);
            } else {
                tx.commit().unwrap();
                rows = candidate;
            }
            assert_eq!(graph.source_rows(&source).unwrap(), &rows);
            if step % 50 == 0 {
                graph.assert_consistent().unwrap();
            }
        }
    }
}
#[test]
fn callbacks_and_later_terminal_errors_restore_whole_graph() {
    let fail_predicate = Cell::new(false);
    let fail_score = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let a = b.source::<usize, Row>();
    let r = b.source::<usize, Row>();
    let exists = b
        .existence_equal(
            a.port(),
            r.port(),
            Mode::NotExists,
            |a| a.0,
            |b| {
                assert!(!fail_predicate.get());
                b.0
            },
        )
        .unwrap();
    let terminal = b
        .score("absence", &exists, |a| {
            if fail_score.get() {
                SimpleScore::new(a.1).with_structural(-1)
            } else {
                SimpleScore::new(a.1)
            }
        })
        .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply(&a, Change::put(0, (0, 3))).unwrap();
    tx.commit().unwrap();
    let snapshot = graph.constraint_matches(&terminal).unwrap().clone();
    fail_predicate.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut tx = graph.transaction();
            tx.apply(&r, Change::put(0, (0, 4))).unwrap();
        }))
        .is_err()
    );
    fail_predicate.set(false);
    assert_eq!(graph.constraint_matches(&terminal).unwrap(), &snapshot);
    assert!(graph.source_rows(&r).unwrap().is_empty());
    fail_score.set(true);
    {
        let mut tx = graph.transaction();
        tx.apply(&r, Change::put(0, (0, 4))).unwrap();
        assert!(tx.apply(&r, Change::retract(0)).is_err());
        assert!(tx.commit().is_err());
    }
    fail_score.set(false);
    assert_eq!(graph.constraint_matches(&terminal).unwrap(), &snapshot);
    assert!(graph.source_rows(&r).unwrap().is_empty());
    graph.assert_consistent().unwrap();
}
#[derive(Debug)]
struct CloneRow(Rc<Cell<bool>>);
impl Clone for CloneRow {
    fn clone(&self) -> Self {
        assert!(!self.0.get(), "clone failure");
        Self(self.0.clone())
    }
}
#[test]
fn output_clone_panic_preserves_standalone_node_and_match_links() {
    let fail = Rc::new(Cell::new(false));
    let mut node =
        ExistsEqualNode::<usize, usize, CloneRow, (), ()>::new(Mode::Exists, |_| (), |_| ());
    node.apply_left(Change::put(0, CloneRow(fail.clone())));
    fail.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| node.apply_right(Change::put(0, ())))).is_err());
    fail.set(false);
    assert_eq!(node.rows().count(), 0);
    assert_eq!(node.apply_right(Change::put(0, ())).len(), 1);
    assert_eq!(node.apply_right(Change::retract(0)).len(), 1);
    assert_eq!(node.rows().count(), 0);
}
#[derive(Clone, Debug)]
struct Key(u32, Rc<Cell<Option<usize>>>);
impl PartialEq for Key {
    fn eq(&self, b: &Self) -> bool {
        self.0 == b.0
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, b: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(b))
    }
}
impl Ord for Key {
    fn cmp(&self, b: &Self) -> std::cmp::Ordering {
        if let Some(n) = self.1.get() {
            assert!(n > 0, "comparison failure");
            self.1.set(Some(n - 1));
        }
        self.0.cmp(&b.0)
    }
}
#[test]
fn ordering_panics_leave_persistent_candidate_uninstalled() {
    let mut caught = 0;
    let mut succeeded = 0;
    for budget in 0..160 {
        let countdown = Rc::new(Cell::new(None));
        let mut node = ExistsEqualNode::<usize, usize, u32, u32, Key>::new(
            Mode::Exists,
            |&v| Key(v, countdown.clone()),
            |&v| Key(v, countdown.clone()),
        );
        for id in 0..12 {
            node.apply_left(Change::put(id, id as u32));
            node.apply_right(Change::put(id, id as u32));
        }
        countdown.set(Some(budget));
        let result = catch_unwind(AssertUnwindSafe(|| node.apply_right(Change::put(5, 6))));
        countdown.set(None);
        let expected: Vec<_> = (0..12)
            .filter(|&id| result.is_err() || id != 5)
            .map(|id| (id, id as u32))
            .collect();
        assert_eq!(
            node.rows().map(|(&i, &r)| (i, r)).collect::<Vec<_>>(),
            expected
        );
        if result.is_err() {
            caught += 1;
            assert!(
                node.apply_right(Change::retract(5))
                    .iter()
                    .any(|c| c.id == 5 && c.value.is_none())
            );
        } else {
            succeeded += 1;
        }
    }
    assert!(caught > 10 && succeeded > 0);
}

#[test]
fn duplicate_rights_and_two_key_transfers_emit_exact_transitions() {
    for mode in [Mode::Exists, Mode::NotExists] {
        let mut n = ExistsEqualNode::new(mode, |a: &Row| a.0, |b: &Row| b.0);
        n.apply_left(Change::put(5, (0, 9)));
        n.apply_left(Change::put(1, (1, 8)));
        let event = |id, row, yes| {
            if yes == (mode == Mode::Exists) {
                Change::put(id, row)
            } else {
                Change::retract(id)
            }
        };
        assert_eq!(
            n.apply_right(Change::put(0, (0, 2))),
            vec![event(5, (0, 9), true)]
        );
        assert!(n.apply_right(Change::put(1, (0, 7))).is_empty());
        assert!(n.apply_right(Change::put(1, (0, 99))).is_empty());
        assert_eq!(
            n.apply_right(Change::put(1, (1, 7))),
            vec![event(1, (1, 8), true)]
        );
        assert_eq!(
            n.apply_right(Change::put(0, (1, 6))),
            vec![event(5, (0, 9), false)]
        );
        assert!(n.apply_right(Change::retract(1)).is_empty());
        assert_eq!(
            n.apply_right(Change::put(0, (0, 0))),
            vec![event(1, (1, 8), false), event(5, (0, 9), true)]
        );
        assert!(n.apply_right(Change::retract(99)).is_empty());
    }
}

#[derive(Clone, Debug)]
struct Counted(u32, Rc<Cell<usize>>);
impl PartialEq for Counted {
    fn eq(&self, b: &Self) -> bool {
        self.0 == b.0
    }
}
impl Eq for Counted {}
impl PartialOrd for Counted {
    fn partial_cmp(&self, b: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(b))
    }
}
impl Ord for Counted {
    fn cmp(&self, b: &Self) -> std::cmp::Ordering {
        self.1.set(self.1.get() + 1);
        self.0.cmp(&b.0)
    }
}
#[test]
fn dense_right_changes_do_not_visit_matching_left_rows() {
    let calls = Rc::new(Cell::new(0));
    let mut n = ExistsEqualNode::new(
        Mode::Exists,
        |r: &u32| Counted(*r, calls.clone()),
        |r: &u32| Counted(*r, calls.clone()),
    );
    // 16,384 x 16,384 matching pairs must not be materialized. Only 32,768
    // input occurrences and a single right-key count are retained.
    for i in 0..16384 {
        n.apply_left(Change::put(i, 0));
    }
    assert_eq!(n.apply_right(Change::put(0, 0)).len(), 16384);
    calls.set(0);
    for i in 1..16384 {
        assert!(n.apply_right(Change::put(i, 0)).is_empty());
    }
    for i in 1..16384 {
        assert!(n.apply_right(Change::retract(i)).is_empty());
    }
    assert!(calls.get() < 200000, "{} comparisons", calls.get());
    assert_eq!(n.apply_right(Change::retract(0)).len(), 16384);
    assert_eq!(n.rows().count(), 0);
}

#[test]
fn right_payload_needs_no_clone_and_removal_does_not_reproject() {
    struct Right(u32);
    let project = Cell::new(0);
    let mut n = ExistsEqualNode::new(
        Mode::Exists,
        |r: &u32| *r,
        |r: &Right| {
            project.set(project.get() + 1);
            r.0
        },
    );
    n.apply_left(Change::put(1, 7));
    assert_eq!(
        n.apply_right(Change::put(0, Right(7))),
        vec![Change::put(1, 7)]
    );
    assert!(n.apply_right(Change::put(0, Right(7))).is_empty());
    assert_eq!(n.apply_right(Change::retract(0)), vec![Change::retract(1)]);
    assert_eq!(project.get(), 2);
}

#[test]
fn tuple_optional_keys_and_foreign_ports_keep_identity_and_topology() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let l = b.source::<usize, (Option<i32>, i32)>();
    let r = b.source::<usize, (Option<i32>, i32)>();
    let mut other = StreamGraphBuilder::<SimpleScore>::new();
    let f = other.source::<usize, (Option<i32>, i32)>();
    for (a, bad) in [(l.port(), f.port()), (f.port(), r.port())] {
        assert!(matches!(
            b.existence_equal(a, bad, Mode::Exists, |r| *r, |r| *r),
            Err(timetuck::GraphError::ForeignPort)
        ));
    }
    let e = b
        .existence_equal(l.port(), r.port(), Mode::NotExists, |r| *r, |r| *r)
        .unwrap();
    let terminal = b.score("missing", &e, |_| SimpleScore::new(-1)).unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&l, Change::put(0, (None, 1))).unwrap();
    t.apply(&l, Change::put(1, (Some(0), 1))).unwrap();
    t.apply(&r, Change::put(0, (None, 2))).unwrap();
    assert_eq!(t.score().unwrap(), SimpleScore::new(-2));
    t.apply(&r, Change::put(1, (None, 1))).unwrap();
    assert_eq!(t.score().unwrap(), SimpleScore::new(-1));
    t.commit().unwrap();
    assert_eq!(
        g.constraint_matches(&terminal)
            .unwrap()
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![1]
    );
    g.assert_consistent().unwrap();
}
