//! Original atomic partitioned-join batch regressions; no upstream method credit.
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
type Row = (i32, i32, i64);
type Rows = BTreeMap<usize, Row>;
type Pairs = BTreeMap<(usize, usize), (Row, Row)>;
const RELATIONS: [JoinComparison; 5] = [
    JoinComparison::Equal,
    JoinComparison::LessThan,
    JoinComparison::LessThanOrEqual,
    JoinComparison::GreaterThan,
    JoinComparison::GreaterThanOrEqual,
];
fn scan(l: &Rows, r: &Rows, c: JoinComparison) -> Pairs {
    l.iter()
        .flat_map(|(&i, &a)| {
            r.iter().filter_map(move |(&j, &b)| {
                let accepts = a.0 == b.0
                    && match c {
                        JoinComparison::Equal => a.1 == b.1,
                        JoinComparison::LessThan => a.1 < b.1,
                        JoinComparison::LessThanOrEqual => a.1 <= b.1,
                        JoinComparison::GreaterThan => a.1 > b.1,
                        JoinComparison::GreaterThanOrEqual => a.1 >= b.1,
                    };
                accepts.then_some(((i, j), (a, b)))
            })
        })
        .collect()
}
fn patches(seed: &mut u64) -> Vec<TupleChange<usize, Row>> {
    (0..8)
        .map(|_| {
            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let n = *seed >> 32;
            TupleChange {
                id: (n % 9) as usize,
                value: (!n.is_multiple_of(5)).then_some((
                    (n / 9 % 4) as i32,
                    (n / 36 % 5) as i32,
                    (n / 180 % 19) as i64,
                )),
            }
        })
        .collect()
}
fn update(rows: &mut Rows, changes: &[TupleChange<usize, Row>]) {
    for c in changes {
        if let Some(v) = c.value {
            rows.insert(c.id, v);
        } else {
            rows.remove(&c.id);
        }
    }
}
#[test]
fn final_pairs_and_projection_counts_match_scans_across_partition_changes() {
    for comparison in RELATIONS {
        let calls = Cell::new(0);
        let project = |v: &Row| {
            calls.set(calls.get() + 1);
            (v.0, v.1)
        };
        let mut node = PartitionedJoinNode::new(comparison, project, project);
        let (mut l, mut r) = (Rows::new(), Rows::new());
        let mut seed = 74021;
        for round in 0..180 {
            if round % 13 == 0 {
                let one = TupleChange::put(0, (round % 3, round % 5, round as i64));
                node.apply_left(one.clone());
                update(&mut l, &[one]);
            }
            let before = scan(&l, &r, comparison);
            let (lc, rc) = (patches(&mut seed), patches(&mut seed));
            let touched_l: BTreeSet<_> = lc.iter().map(|c| c.id).collect();
            let touched_r: BTreeSet<_> = rc.iter().map(|c| c.id).collect();
            let expected_calls = [&lc, &rc]
                .into_iter()
                .map(|cs| {
                    cs.iter()
                        .map(|c| (c.id, c.value))
                        .collect::<BTreeMap<_, _>>()
                        .values()
                        .filter(|v| v.is_some())
                        .count()
                })
                .sum::<usize>();
            update(&mut l, &lc);
            update(&mut r, &rc);
            calls.set(0);
            let changes = node.apply_batch(lc, rc);
            assert_eq!(calls.get(), expected_calls);
            let after = scan(&l, &r, comparison);
            let ids: BTreeSet<_> = before
                .keys()
                .chain(after.keys())
                .copied()
                .filter(|(i, j)| touched_l.contains(i) || touched_r.contains(j))
                .collect();
            let expected: Vec<_> = ids
                .into_iter()
                .map(|id| TupleChange {
                    id,
                    value: after.get(&id).copied(),
                })
                .collect();
            assert_eq!(changes, expected);
            assert_eq!(
                node.matches()
                    .map(|((i, j), (a, b))| ((*i, *j), (*a, *b)))
                    .collect::<Pairs>(),
                after
            );
            let prefixes: BTreeSet<_> = l.values().chain(r.values()).map(|v| v.0).collect();
            assert_eq!(node.partition_count(), prefixes.len());
        }
        let lc = l
            .keys()
            .copied()
            .map(TupleChange::retract)
            .collect::<Vec<_>>();
        let rc = r
            .keys()
            .copied()
            .map(TupleChange::retract)
            .collect::<Vec<_>>();
        node.apply_batch(lc, rc);
        assert_eq!(node.partition_count(), 0);
        assert!(node.apply_batch([], []).is_empty());
    }
}
#[test]
fn graph_batches_match_scans_with_pruning_and_rollback() {
    for comparison in RELATIONS {
        for pruning in [false, true] {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            if pruning {
                b = b.with_activity_pruning();
            }
            let left = b.source::<usize, Row>();
            let right = b.source::<usize, Row>();
            let port = b
                .join_partitioned_batched(
                    left.port(),
                    right.port(),
                    comparison,
                    |r| (r.0, r.1),
                    |r| (r.0, r.1),
                )
                .unwrap();
            let data = b.dataset(&port).unwrap();
            b.score("pairs", &port, |(l, r)| SimpleScore::new(l.2 + r.2))
                .unwrap();
            let mut graph = b.build();
            let (mut l, mut r) = (Rows::new(), Rows::new());
            let mut seed = 71981;
            for round in 0..90 {
                let lc = patches(&mut seed);
                let rc = if round % 11 == 0 {
                    (0..9).map(TupleChange::retract).collect()
                } else {
                    patches(&mut seed)
                };
                let (mut nl, mut nr) = (l.clone(), r.clone());
                update(&mut nl, &lc);
                update(&mut nr, &rc);
                let expected = scan(&nl, &nr, comparison);
                let mut batch = StreamBatch::new();
                for c in lc {
                    batch.push(&left, c).unwrap();
                }
                for c in rc {
                    batch.push(&right, c).unwrap();
                }
                let mut tx = graph.transaction();
                tx.apply_batch(batch).unwrap();
                assert_eq!(tx.graph().unwrap().dataset_rows(&data).unwrap(), &expected);
                assert_eq!(
                    tx.score().unwrap(),
                    SimpleScore::new(expected.values().map(|(l, r)| l.2 + r.2).sum())
                );
                if round % 17 == 0 {
                    tx.assert_consistent().unwrap();
                }
                if round % 3 == 0 {
                    drop(tx);
                } else {
                    tx.commit().unwrap();
                    l = nl;
                    r = nr;
                }
                assert_eq!(
                    graph.dataset_rows(&data).unwrap(),
                    &scan(&l, &r, comparison)
                );
            }
            graph.assert_consistent().unwrap();
        }
    }
}
#[test]
fn intermediate_pairs_are_never_materialized_and_sides_stay_lazy() {
    let calls = Cell::new(0);
    let clones = Rc::new(Cell::new(0));
    struct Counted(i32, Rc<Cell<usize>>);
    impl Clone for Counted {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            Self(self.0, self.1.clone())
        }
    }
    let mut node = PartitionedJoinNode::new(
        JoinComparison::Equal,
        |r: &Counted| {
            calls.set(calls.get() + 1);
            (r.0, ())
        },
        |r: &Counted| {
            calls.set(calls.get() + 1);
            (r.0, ())
        },
    );
    let row = |v| Counted(v, clones.clone());
    assert!(
        node.apply_batch([TupleChange::put(0, row(0))], [])
            .is_empty()
    );
    assert!(!node.partition_state(&0).unwrap().right_initialized);
    node.apply_right(TupleChange::put(0, row(0)));
    clones.set(0);
    calls.set(0);
    let output = node.apply_batch(
        [TupleChange::put(0, row(8)), TupleChange::put(0, row(1))],
        [TupleChange::put(0, row(7)), TupleChange::put(0, row(1))],
    );
    assert_eq!(calls.get(), 2);
    assert_eq!(clones.get(), 2); // Only the one final pair.
    assert_eq!(output.len(), 1);
    assert_eq!(output[0].id, (0, 0));
    let pair = output[0].value.as_ref().unwrap();
    assert_eq!((pair.0.0, pair.1.0), (1, 1));
    assert!(node.partition_state(&0).is_none());
    assert_eq!(node.partition_count(), 1);
}
#[test]
fn input_projection_and_output_clone_panics_preserve_standalone_state() {
    struct Value(i32, Rc<Cell<bool>>);
    impl Clone for Value {
        fn clone(&self) -> Self {
            assert!(!self.1.get(), "clone failure");
            Self(self.0, self.1.clone())
        }
    }
    let clone_fail = Rc::new(Cell::new(false));
    let project_fail = Cell::new(false);
    let mut node = PartitionedJoinNode::new(
        JoinComparison::Equal,
        |r: &Value| {
            assert!(!project_fail.get(), "projection failure");
            (r.0, ())
        },
        |r: &Value| (r.0, ()),
    );
    let row = |v| Value(v, clone_fail.clone());
    node.apply_batch([TupleChange::put(0, row(0))], [TupleChange::put(0, row(0))]);
    project_fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || node.apply_batch([TupleChange::put(0, row(1))], [])
        ))
        .is_err()
    );
    project_fail.set(false);
    clone_fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || node.apply_batch([TupleChange::put(0, row(0))], [])
        ))
        .is_err()
    );
    clone_fail.set(false);
    assert!(
        catch_unwind(AssertUnwindSafe(|| node.apply_batch(
            std::iter::once(TupleChange::put(0, row(1)))
                .chain(std::iter::once_with(|| panic!("iterator failure"))),
            []
        )))
        .is_err()
    );
    assert_eq!(
        node.matches()
            .map(|((i, j), (l, r))| (*i, *j, l.0, r.0))
            .collect::<Vec<_>>(),
        vec![(0, 0, 0, 0)]
    );
    node.apply_batch([TupleChange::put(0, row(1))], [TupleChange::put(0, row(1))]);
    assert_eq!(
        node.matches()
            .map(|((i, j), (l, r))| (*i, *j, l.0, r.0))
            .collect::<Vec<_>>(),
        vec![(0, 0, 1, 1)]
    );
}
#[test]
fn batched_partitioned_self_join_drives_solver_moves_and_saved_candidates() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0i32, 1]).with_initial(0); 4]).unwrap();
    let mut network = ConstraintNetwork::new(4);
    for scope in [vec![0, 2], vec![1, 3]] {
        network
            .add_constraint(format!("conflict{}", scope[0]), scope, |v| {
                SimpleScore::new(-i64::from(v[0] == v[1]))
            })
            .unwrap();
    }
    let config = LocalSearchConfig {
        max_steps: 4,
        assert_scores: true,
        ..Default::default()
    };
    let full = local_search_from(&p, &network, p.initial_assignment(), config.clone()).unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let source = b.source::<usize, (usize, Option<i32>)>();
    let pairs = b
        .join_partitioned_batched(
            source.port(),
            source.port(),
            JoinComparison::Equal,
            |r| (r.0 % 2, r.1),
            |r| (r.0 % 2, r.1),
        )
        .unwrap();
    let unique = b.filter(&pairs, |(l, r)| l.0 < r.0).unwrap();
    b.score("conflict", &unique, |_| SimpleScore::new(-1))
        .unwrap();
    let result =
        stream_local_search_from(&p, b.build(), source, p.initial_assignment(), config).unwrap();
    assert_eq!(result.best.as_ref().unwrap().score, SimpleScore::ZERO);
    assert_eq!(result.best, full.best);
}

#[test]
fn partition_exchange_does_not_score_a_transient_extra_pair() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let left = b.source::<usize, i32>();
    let right = b.source::<usize, i32>();
    let port = b
        .join_partitioned_batched(
            left.port(),
            right.port(),
            JoinComparison::Equal,
            |v| (*v, ()),
            |v| (*v, ()),
        )
        .unwrap();
    b.score("one pair", &port, |_| SimpleScore::new(i64::MAX))
        .unwrap();
    let mut g = b.build();
    {
        let mut tx = g.transaction();
        tx.apply(&left, TupleChange::put(0, 0)).unwrap();
        tx.apply(&right, TupleChange::put(0, 0)).unwrap();
        tx.apply(&right, TupleChange::put(1, 1)).unwrap();
        tx.commit().unwrap();
    }
    let mut batch = StreamBatch::new();
    batch.push(&left, TupleChange::put(0, 1)).unwrap();
    batch.push(&right, TupleChange::put(0, 1)).unwrap();
    batch.push(&right, TupleChange::retract(1)).unwrap();
    {
        let mut tx = g.transaction();
        tx.apply_batch(batch).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(i64::MAX));
        tx.assert_consistent().unwrap();
    }
    assert_eq!(g.score(), SimpleScore::new(i64::MAX));
    assert_eq!(g.source_rows(&left).unwrap()[&0], 0);
    assert_eq!(g.source_rows(&right).unwrap()[&1], 1);
}
