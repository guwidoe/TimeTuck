//! Original pair-free ordered existence regressions; no upstream method credits.
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
type Row = (i32, i32, i64);
type Rows = BTreeMap<usize, Row>;
const RELATIONS: [JoinComparison; 5] = [
    JoinComparison::Equal,
    JoinComparison::LessThan,
    JoinComparison::LessThanOrEqual,
    JoinComparison::GreaterThan,
    JoinComparison::GreaterThanOrEqual,
];
fn scan(left: &Rows, right: &Rows, c: JoinComparison, m: ExistenceMode) -> Rows {
    left.iter()
        .filter(|(_, l)| {
            right.values().any(|r| {
                l.0 == r.0
                    && match c {
                        JoinComparison::Equal => l.1 == r.1,
                        JoinComparison::LessThan => l.1 < r.1,
                        JoinComparison::LessThanOrEqual => l.1 <= r.1,
                        JoinComparison::GreaterThan => l.1 > r.1,
                        JoinComparison::GreaterThanOrEqual => l.1 >= r.1,
                    }
            }) == (m == ExistenceMode::Exists)
        })
        .map(|(&i, &r)| (i, r))
        .collect()
}
fn patches(seed: &mut u64) -> Vec<TupleChange<usize, Row>> {
    (0..7)
        .map(|_| {
            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let n = *seed >> 32;
            TupleChange {
                id: (n % 19) as usize,
                value: (!n.is_multiple_of(4)).then_some((
                    (n / 19 % 4) as i32,
                    (n / 76 % 7) as i32,
                    (n / 532 % 31) as i64 - 15,
                )),
            }
        })
        .collect()
}
fn update(rows: &mut Rows, cs: &[TupleChange<usize, Row>]) {
    for c in cs {
        if let Some(r) = c.value {
            rows.insert(c.id, r);
        } else {
            rows.remove(&c.id);
        }
    }
}
#[test]
fn final_visibility_matches_scans_for_every_relation_and_mode() {
    for c in RELATIONS {
        for mode in [ExistenceMode::Exists, ExistenceMode::NotExists] {
            let calls = Cell::new(0);
            let project = |r: &Row| {
                calls.set(calls.get() + 1);
                (r.0, r.1)
            };
            let mut n = ExistsCompareNode::new(mode, c, project, project);
            let (mut l, mut r) = (Rows::new(), Rows::new());
            let mut seed = 723809;
            for step in 0..220 {
                let lc = patches(&mut seed);
                let rc = if step % 11 == 0 {
                    (0..19).map(TupleChange::retract).collect()
                } else {
                    patches(&mut seed)
                };
                let before = scan(&l, &r, c, mode);
                let touched: BTreeSet<_> = lc.iter().map(|v| v.id).collect();
                let count = [&lc, &rc]
                    .into_iter()
                    .map(|cs| {
                        cs.iter()
                            .map(|v| (v.id, v.value))
                            .collect::<BTreeMap<_, _>>()
                            .values()
                            .filter(|v| v.is_some())
                            .count()
                    })
                    .sum::<usize>();
                update(&mut l, &lc);
                update(&mut r, &rc);
                let after = scan(&l, &r, c, mode);
                calls.set(0);
                let output = n.apply_batch(lc, rc);
                assert_eq!(calls.get(), count);
                let ids: BTreeSet<_> = before.keys().chain(after.keys()).copied().collect();
                let expected: Vec<_> = ids
                    .into_iter()
                    .filter_map(|id| match after.get(&id) {
                        Some(row) if !before.contains_key(&id) || touched.contains(&id) => {
                            Some(TupleChange::put(id, *row))
                        }
                        None if before.contains_key(&id) => Some(TupleChange::retract(id)),
                        _ => None,
                    })
                    .collect();
                assert_eq!(output, expected, "{c:?} {mode:?} step {step}");
                assert_eq!(n.rows().map(|(&i, &v)| (i, v)).collect::<Rows>(), after);
                assert_eq!(
                    n.partition_count(),
                    l.values()
                        .chain(r.values())
                        .map(|v| v.0)
                        .collect::<BTreeSet<_>>()
                        .len()
                );
            }
            n.apply_batch(
                l.keys().copied().map(TupleChange::retract),
                r.keys().copied().map(TupleChange::retract),
            );
            assert_eq!(n.partition_count(), 0);
            assert!(n.apply_batch([], []).is_empty());
        }
    }
}
#[test]
fn graphs_restore_batches_with_pruning_and_self_joins() {
    for c in RELATIONS {
        for mode in [ExistenceMode::Exists, ExistenceMode::NotExists] {
            for pruning in [false, true] {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                if pruning {
                    b = b.with_activity_pruning();
                }
                let left = b.source::<usize, Row>();
                let right = b.source::<usize, Row>();
                let joined = b
                    .existence_partitioned_batched(
                        left.port(),
                        right.port(),
                        mode,
                        c,
                        |v| (v.0, v.1),
                        |v| (v.0, v.1),
                    )
                    .unwrap();
                let self_join = b
                    .existence_compare_batched(left.port(), left.port(), mode, c, |v| v.1, |v| v.1)
                    .unwrap();
                let data = b.dataset(&joined).unwrap();
                let self_data = b.dataset(&self_join).unwrap();
                b.score("exists", &joined, |r| SimpleScore::new(r.2))
                    .unwrap();
                let mut g = b.build();
                let (mut l, mut r) = (Rows::new(), Rows::new());
                let mut seed = 8024;
                for step in 0..75 {
                    let (lc, rc) = (patches(&mut seed), patches(&mut seed));
                    let (mut nl, mut nr) = (l.clone(), r.clone());
                    update(&mut nl, &lc);
                    update(&mut nr, &rc);
                    let expected = scan(&nl, &nr, c, mode);
                    let unpartitioned: Rows =
                        nl.iter().map(|(&i, &(_, k, v))| (i, (0, k, v))).collect();
                    let self_expected: Rows = scan(&unpartitioned, &unpartitioned, c, mode)
                        .keys()
                        .map(|i| (*i, nl[i]))
                        .collect();
                    let mut batch = StreamBatch::new();
                    for v in lc {
                        batch.push(&left, v).unwrap();
                    }
                    for v in rc {
                        batch.push(&right, v).unwrap();
                    }
                    let mut tx = g.transaction();
                    tx.apply_batch(batch).unwrap();
                    assert_eq!(tx.graph().unwrap().dataset_rows(&data).unwrap(), &expected);
                    assert_eq!(
                        tx.graph().unwrap().dataset_rows(&self_data).unwrap(),
                        &self_expected
                    );
                    assert_eq!(
                        tx.score().unwrap(),
                        SimpleScore::new(expected.values().map(|r| r.2).sum())
                    );
                    if step % 15 == 0 {
                        tx.assert_consistent().unwrap();
                    }
                    if step % 3 == 0 {
                        drop(tx);
                    } else {
                        tx.commit().unwrap();
                        l = nl;
                        r = nr;
                    }
                    assert_eq!(g.dataset_rows(&data).unwrap(), &scan(&l, &r, c, mode));
                }
                g.assert_consistent().unwrap();
            }
        }
    }
}
#[test]
fn duplicate_witnesses_and_unchanged_boundaries_do_not_clone_left_rows() {
    struct RowKey(i32, Rc<Cell<usize>>);
    impl Clone for RowKey {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            Self(self.0, self.1.clone())
        }
    }
    struct Witness(i32); // Deliberately not Clone; payloads are never retained.
    for c in RELATIONS {
        let clones = Rc::new(Cell::new(0));
        let mut n = ExistsCompareNode::new(
            ExistenceMode::Exists,
            c,
            |r: &RowKey| ((), r.0),
            |r: &Witness| ((), r.0),
        );
        n.apply_batch(
            (0..4000).map(|i| TupleChange::put(i, RowKey(i, clones.clone()))),
            (0..4000).map(|i| TupleChange::put(i, Witness(2000))),
        );
        clones.set(0);
        assert!(
            n.apply_right(TupleChange::put(4000, Witness(2000)))
                .is_empty()
        );
        assert!(n.apply_right(TupleChange::retract(0)).is_empty());
        assert_eq!(clones.get(), 0);
        let out = n.apply_batch([], (1..4001).map(TupleChange::retract));
        assert!(out.iter().all(|v| v.value.is_none()));
        assert_eq!(n.rows().count(), 0);
        assert_eq!(clones.get(), 0);
    }
}
#[test]
fn only_crossed_left_keys_are_visited() {
    #[derive(Clone, Eq, PartialEq)]
    struct Key(i32, Rc<Cell<usize>>);
    impl Ord for Key {
        fn cmp(&self, b: &Self) -> std::cmp::Ordering {
            self.1.set(self.1.get() + 1);
            self.0.cmp(&b.0)
        }
    }
    impl PartialOrd for Key {
        fn partial_cmp(&self, b: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(b))
        }
    }
    let comparisons = Rc::new(Cell::new(0));
    let mut n = ExistsCompareNode::new(
        ExistenceMode::Exists,
        JoinComparison::LessThan,
        |r: &i32| ((), Key(*r, comparisons.clone())),
        |r: &i32| ((), Key(*r, comparisons.clone())),
    );
    n.apply_batch(
        (0..10000).map(|i| TupleChange::put(i, i)),
        [TupleChange::put(0, 5000)],
    );
    comparisons.set(0);
    assert_eq!(
        n.apply_right(TupleChange::put(0, 5001)),
        vec![TupleChange::put(5000, 5000)]
    );
    assert!(
        comparisons.get() < 300,
        "compared {} keys",
        comparisons.get()
    );
}
#[test]
fn failed_preparation_and_downstream_scoring_restore_inputs_and_outputs() {
    let fail = Cell::new(false);
    let mut n = ExistsCompareNode::new(
        ExistenceMode::NotExists,
        JoinComparison::GreaterThan,
        |r: &i32| (0, *r),
        |r: &i32| {
            assert!(!fail.get());
            (0, *r)
        },
    );
    n.apply_left(TupleChange::put(0, 5));
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || n.apply_batch([TupleChange::put(0, 8)], [TupleChange::put(0, 2)])
        ))
        .is_err()
    );
    fail.set(false);
    assert_eq!(
        n.rows().map(|(&i, &v)| (i, v)).collect::<Vec<_>>(),
        vec![(0, 5)]
    );
    assert_eq!(
        n.apply_right(TupleChange::put(0, 2)),
        vec![TupleChange::retract(0)]
    );
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let left = b.source::<usize, i32>();
    let right = b.source::<usize, i32>();
    let p = b
        .existence_compare_batched(
            left.port(),
            right.port(),
            ExistenceMode::NotExists,
            JoinComparison::LessThan,
            |v| *v,
            |v| *v,
        )
        .unwrap();
    b.score("visible", &p, |_| {
        assert!(!fail.get());
        SimpleScore::new(1)
    })
    .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&right, TupleChange::put(0, 10)).unwrap();
    tx.apply(&left, TupleChange::put(0, 5)).unwrap();
    tx.commit().unwrap();
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut tx = g.transaction();
            tx.apply(&right, TupleChange::retract(0)).unwrap();
            tx.commit().unwrap();
        }))
        .is_err()
    );
    fail.set(false);
    assert_eq!(g.source_rows(&right).unwrap()[&0], 10);
    assert_eq!(g.score(), SimpleScore::ZERO);
    g.assert_consistent().unwrap();
}

#[test]
fn ordered_existence_drives_search_and_saved_candidate_replay() {
    let p = FiniteProblem::new(
        (0..6)
            .map(|i| Variable::new(vec![0i32, 1, 2]).with_initial(i % 3))
            .collect(),
    )
    .unwrap();
    for mode in [ExistenceMode::Exists, ExistenceMode::NotExists] {
        let mut network = ConstraintNetwork::new(6);
        for partition in 0..2 {
            network
                .add_constraint(
                    format!("group{partition}"),
                    (partition..6).step_by(2).collect(),
                    move |v| {
                        SimpleScore::new(
                            -(v.iter()
                                .filter(|a| {
                                    v.iter().any(|b| *a < b) == (mode == ExistenceMode::Exists)
                                })
                                .count() as i64),
                        )
                    },
                )
                .unwrap();
        }
        let config = LocalSearchConfig {
            max_steps: 12,
            assert_scores: true,
            ..Default::default()
        };
        let full = local_search_from(&p, &network, p.initial_assignment(), config.clone()).unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
        let source = b.source::<usize, (usize, Option<i32>)>();
        let visible = b
            .existence_partitioned_batched(
                source.port(),
                source.port(),
                mode,
                JoinComparison::LessThan,
                |r| (r.0 % 2, r.1),
                |r| (r.0 % 2, r.1),
            )
            .unwrap();
        b.score("visibility", &visible, |_| SimpleScore::new(-1))
            .unwrap();
        let actual =
            stream_local_search_from(&p, b.build(), source, p.initial_assignment(), config)
                .unwrap();
        assert_eq!(actual.best, full.best);
        assert_eq!(
            actual.best.unwrap().score,
            SimpleScore::new(if mode == ExistenceMode::Exists { 0 } else { -2 })
        );
    }
}
