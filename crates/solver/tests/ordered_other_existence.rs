//! Original identity-excluding ordered existence regressions; no upstream credits.
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
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
        .filter(|(i, l)| {
            right.iter().any(|(j, r)| {
                *i != j
                    && l.0 == r.0
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
fn other_visibility_matches_independent_scans_for_all_relations() {
    for c in RELATIONS {
        for mode in [ExistenceMode::Exists, ExistenceMode::NotExists] {
            let calls = Cell::new(0);
            let project = |r: &Row| {
                calls.set(calls.get() + 1);
                (r.0, r.1)
            };
            let mut n = ExistsCompareNode::new_other(mode, c, project, project);
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
fn other_graphs_restore_filtered_identity_streams_and_self_joins() {
    for c in RELATIONS {
        for mode in [ExistenceMode::Exists, ExistenceMode::NotExists] {
            for pruning in [false, true] {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                if pruning {
                    b = b.with_activity_pruning();
                }
                let left = b.source::<usize, Row>();
                let right = b.source::<usize, Row>();
                let eligible = b.filter(right.port(), |r| r.2 >= 0).unwrap();
                let joined = b
                    .existence_other_partitioned_batched(
                        left.port(),
                        &eligible,
                        mode,
                        c,
                        |v| (v.0, v.1),
                        |v| (v.0, v.1),
                    )
                    .unwrap();
                let self_join = b
                    .existence_other_compare_batched(
                        left.port(),
                        left.port(),
                        mode,
                        c,
                        |v| v.1,
                        |v| v.1,
                    )
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
                    let expected = scan(
                        &nl,
                        &nr.iter()
                            .filter(|(_, r)| r.2 >= 0)
                            .map(|(&i, &r)| (i, r))
                            .collect(),
                        c,
                        mode,
                    );
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
                    assert_eq!(
                        g.dataset_rows(&data).unwrap(),
                        &scan(
                            &l,
                            &r.iter()
                                .filter(|(_, r)| r.2 >= 0)
                                .map(|(&i, &r)| (i, r))
                                .collect(),
                            c,
                            mode
                        )
                    );
                }
                g.assert_consistent().unwrap();
            }
        }
    }
}

#[test]
fn unchanged_extrema_still_track_the_second_witness_and_identity_changes() {
    for mode in [ExistenceMode::Exists, ExistenceMode::NotExists] {
        for c in [
            JoinComparison::LessThan,
            JoinComparison::LessThanOrEqual,
            JoinComparison::GreaterThan,
            JoinComparison::GreaterThanOrEqual,
        ] {
            let flip = if matches!(
                c,
                JoinComparison::GreaterThan | JoinComparison::GreaterThanOrEqual
            ) {
                -1
            } else {
                1
            };
            let mut n =
                ExistsCompareNode::new_other(mode, c, |v: &i32| ((), *v), |v: &i32| ((), *v));
            n.apply_batch(
                [TupleChange::put(0, 5 * flip)],
                [
                    TupleChange::put(0, 10 * flip),
                    TupleChange::put(1, 4 * flip),
                ],
            );
            assert_eq!(
                n.rows().count(),
                usize::from(mode == ExistenceMode::NotExists)
            );
            assert_eq!(
                n.apply_right(TupleChange::put(1, 6 * flip)),
                vec![if mode == ExistenceMode::Exists {
                    TupleChange::put(0, 5 * flip)
                } else {
                    TupleChange::retract(0)
                }]
            );
            assert_eq!(
                n.apply_right(TupleChange::retract(1)),
                vec![if mode == ExistenceMode::NotExists {
                    TupleChange::put(0, 5 * flip)
                } else {
                    TupleChange::retract(0)
                }]
            );
        }
        let mut n = ExistsCompareNode::new_other(
            mode,
            JoinComparison::Equal,
            |v: &i32| ((), *v),
            |v: &i32| ((), *v),
        );
        n.apply_batch(
            [TupleChange::put(0, 7), TupleChange::put(1, 7)],
            [TupleChange::put(0, 7)],
        );
        assert_eq!(
            n.rows().map(|(&i, _)| i).collect::<Vec<_>>(),
            vec![usize::from(mode == ExistenceMode::Exists)]
        );
        let out = n.apply_batch([], [TupleChange::retract(0), TupleChange::put(1, 7)]);
        assert_eq!(
            out,
            if mode == ExistenceMode::Exists {
                vec![TupleChange::put(0, 7), TupleChange::retract(1)]
            } else {
                vec![TupleChange::retract(0), TupleChange::put(1, 7)]
            }
        );
    }
}
#[test]
fn losing_a_duplicate_witness_does_not_scan_its_entire_left_bucket() {
    use std::rc::Rc;
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
    for c in [
        JoinComparison::Equal,
        JoinComparison::LessThanOrEqual,
        JoinComparison::GreaterThanOrEqual,
    ] {
        let comparisons = Rc::new(Cell::new(0));
        let mut n = ExistsCompareNode::new_other(
            ExistenceMode::Exists,
            c,
            |v: &i32| ((), Key(*v, comparisons.clone())),
            |v: &i32| ((), Key(*v, comparisons.clone())),
        );
        n.apply_batch(
            (0..10000).map(|i| TupleChange::put(i, 1)),
            [TupleChange::put(0, 1), TupleChange::put(1, 1)],
        );
        comparisons.set(0);
        assert_eq!(
            n.apply_right(TupleChange::retract(1)),
            vec![TupleChange::retract(0)]
        );
        assert!(
            comparisons.get() < 300,
            "{c:?}: {} comparisons",
            comparisons.get()
        );
        assert_eq!(n.rows().count(), 9999);
    }
}
#[test]
fn other_witnesses_drive_scored_moves_and_saved_candidates() {
    for mode in [ExistenceMode::Exists, ExistenceMode::NotExists] {
        let p = FiniteProblem::new(
            (0..6)
                .map(|i| {
                    Variable::new(vec![0i32, 1, 2]).with_initial(if mode == ExistenceMode::Exists {
                        0
                    } else {
                        i % 3
                    })
                })
                .collect(),
        )
        .unwrap();
        let mut net = ConstraintNetwork::new(6);
        for partition in 0..2 {
            net.add_constraint(
                format!("group{partition}"),
                (partition..6).step_by(2).collect(),
                move |v| {
                    SimpleScore::new(
                        -(v.iter()
                            .enumerate()
                            .filter(|(i, a)| {
                                v.iter().enumerate().any(|(j, b)| *i != j && *a == b)
                                    == (mode == ExistenceMode::Exists)
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
        let full = local_search_from(&p, &net, p.initial_assignment(), config.clone()).unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
        let source = b.source::<usize, (usize, Option<i32>)>();
        let visible = b
            .existence_other_partitioned_batched(
                source.port(),
                source.port(),
                mode,
                JoinComparison::Equal,
                |r| (r.0 % 2, r.1),
                |r| (r.0 % 2, r.1),
            )
            .unwrap();
        b.score("other", &visible, |_| SimpleScore::new(-1))
            .unwrap();
        let actual =
            stream_local_search_from(&p, b.build(), source, p.initial_assignment(), config)
                .unwrap();
        assert_eq!(actual.best, full.best);
        assert_eq!(actual.best.unwrap().score, SimpleScore::ZERO);
    }
}

#[test]
fn output_clone_failure_keeps_witness_membership_and_allows_retry() {
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };
    struct Value(i32, Rc<Cell<bool>>);
    impl Clone for Value {
        fn clone(&self) -> Self {
            assert!(!self.1.get());
            Self(self.0, self.1.clone())
        }
    }
    let fail = Rc::new(Cell::new(false));
    let mut n = ExistsCompareNode::new_other(
        ExistenceMode::Exists,
        JoinComparison::Equal,
        |v: &Value| ((), v.0),
        |v: &i32| ((), *v),
    );
    n.apply_batch(
        [TupleChange::put(0, Value(7, fail.clone()))],
        [TupleChange::put(0, 7)],
    );
    assert_eq!(n.rows().count(), 0);
    fail.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| n.apply_right(TupleChange::put(1, 7)))).is_err());
    fail.set(false);
    assert_eq!(n.rows().count(), 0);
    let out = n.apply_right(TupleChange::put(1, 7));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].value.as_ref().unwrap().0, 7);
    assert!(n.apply_right(TupleChange::retract(1))[0].value.is_none());
    assert_eq!(n.rows().count(), 0);
}
