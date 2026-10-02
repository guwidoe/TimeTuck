//! Original regressions; no upstream coverage credit.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::{
    ExistenceMode as Mode, ExistsNode, SimpleScore, StreamGraphBuilder, TupleChange as Change,
};

type Row = (u32, i64);
fn predicate(a: &Row, b: &Row) -> bool {
    a.1 <= b.1
}
fn random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed >> 32
}
#[test]
fn indexed_edits_match_independent_nested_enumeration() {
    for mode in [Mode::Exists, Mode::NotExists] {
        let mut node =
            ExistsNode::<usize, usize, Row, Row, u32>::new(mode, |a| a.0, |b| b.0, predicate);
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
fn multiplicity_and_replacements_emit_only_visibility_transitions() {
    let calls = Cell::new(0);
    let mut node = ExistsNode::<usize, usize, Row, Row, u32>::new(
        Mode::Exists,
        |a| a.0,
        |b| b.0,
        |a, b| {
            calls.set(calls.get() + 1);
            predicate(a, b)
        },
    );
    for i in 0..64 {
        assert!(node.apply_left(Change::put(i, (i as u32, 0))).is_none());
    }
    assert_eq!(
        node.apply_right(Change::put(0, (5, 1))),
        vec![Change::put(5, (5, 0))]
    );
    assert_eq!(calls.get(), 1);
    assert!(node.apply_right(Change::put(1, (5, 2))).is_empty());
    assert!(node.apply_right(Change::put(1, (5, 3))).is_empty());
    let before = calls.get();
    assert!(node.apply_right(Change::retract(0)).is_empty());
    assert_eq!(
        node.apply_right(Change::retract(1)),
        vec![Change::retract(5)]
    );
    assert!(node.apply_right(Change::retract(1)).is_empty());
    assert_eq!(calls.get(), before);
    node.apply_right(Change::put(2, (5, 10)));
    assert_eq!(
        node.apply_left(Change::put(5, (5, 4))),
        Some(Change::put(5, (5, 4)))
    );
    let before = calls.get();
    assert_eq!(
        node.apply_left(Change::retract(5)),
        Some(Change::retract(5))
    );
    assert!(node.apply_right(Change::retract(2)).is_empty());
    assert_eq!(calls.get(), before);
}
#[test]
fn self_existence_graph_commits_and_rollbacks_match_full_scores() {
    for mode in [Mode::Exists, Mode::NotExists] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, u32, i64)>();
        let exists = b
            .existence(
                source.port(),
                source.port(),
                mode,
                |a| a.1,
                |b| b.1,
                |a, b| a.0 != b.0 && a.2 <= b.2,
            )
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
                .filter(|(_, a)| {
                    candidate
                        .values()
                        .any(|b| a.0 != b.0 && a.1 == b.1 && a.2 <= b.2)
                        == (mode == Mode::Exists)
                })
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
        .existence(
            a.port(),
            r.port(),
            Mode::NotExists,
            |a| a.0,
            |b| b.0,
            |a, b| {
                assert!(!fail_predicate.get());
                predicate(a, b)
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
    let mut node = ExistsNode::<usize, usize, CloneRow, (), ()>::new(
        Mode::Exists,
        |_| (),
        |_| (),
        |_, _| true,
    );
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
        let mut node = ExistsNode::<usize, usize, u32, u32, Key>::new(
            Mode::Exists,
            |&v| Key(v, countdown.clone()),
            |&v| Key(v, countdown.clone()),
            |_, _| true,
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
fn existence_scoring_integrates_with_construction_and_local_search() {
    use timetuck::{
        ConstructionConfig, FiniteProblem, LocalSearchConfig, Variable, greedy_stream_construction,
        stream_local_search,
    };
    let make_graph = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let jobs = b.source::<usize, (usize, Option<usize>)>();
        let assigned = b.filter(jobs.port(), |(_, value)| value.is_some()).unwrap();
        let conflicts = b
            .existence(
                &assigned,
                &assigned,
                Mode::Exists,
                |a| a.1,
                |b| b.1,
                |a, b| a.0 != b.0,
            )
            .unwrap();
        b.score("shared machine", &conflicts, |_| SimpleScore::new(-1))
            .unwrap();
        (b.build(), jobs)
    };
    let unassigned =
        FiniteProblem::new((0..3).map(|_| Variable::new(vec![0, 1, 2])).collect()).unwrap();
    let (graph, source) = make_graph();
    let construction = greedy_stream_construction(
        &unassigned,
        graph,
        source,
        ConstructionConfig {
            assert_scores: true,
            ..ConstructionConfig::default()
        },
    )
    .unwrap();
    assert!(unassigned.is_complete(&construction.assignment));
    assert_eq!(construction.score, Some(SimpleScore::ZERO));
    let problem = FiniteProblem::new(
        (0..3)
            .map(|_| Variable::new(vec![0, 1, 2]).with_initial(0))
            .collect(),
    )
    .unwrap();
    let (graph, source) = make_graph();
    let result = stream_local_search(
        &problem,
        graph,
        source,
        LocalSearchConfig {
            assert_scores: true,
            max_evaluations: 100,
            max_steps: 10,
            ..LocalSearchConfig::default()
        },
    )
    .unwrap();
    let best = result.best.unwrap();
    assert_eq!(best.score, SimpleScore::ZERO);
    let values = best.assignment.values();
    for (i, value) in values.iter().enumerate() {
        assert!(value.is_some());
        assert_eq!(
            values
                .iter()
                .enumerate()
                .filter(|(j, other)| *j != i && *other == value)
                .count(),
            0
        );
    }
}
