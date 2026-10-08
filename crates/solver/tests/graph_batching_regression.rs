//! Original transactional batch routing and solver integration regressions.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn heterogeneous_batches_match_pair_scan_through_activity_changes_and_rollback() {
    type Row = (i32, i64);
    for pruning in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let left = b.source::<usize, Row>();
        let right = b.source::<usize, Row>();
        let pairs = b
            .join_batched(
                left.port(),
                right.port(),
                |r| r.0,
                |r| r.0,
                |l, r| l.1 < r.1,
            )
            .unwrap();
        let data = b.dataset(&pairs).unwrap();
        b.score("pairs", &pairs, |(l, r)| SimpleScore::new(r.1 - l.1))
            .unwrap();
        let mut g = b.build();
        let mut l = BTreeMap::new();
        let mut r = BTreeMap::new();
        let mut seed = 175931u64;
        let expected = |l: &BTreeMap<usize, Row>, r: &BTreeMap<usize, Row>| {
            let mut out = BTreeMap::new();
            for (i, a) in l {
                for (j, b) in r {
                    if a.0 == b.0 && a.1 < b.1 {
                        out.insert((*i, *j), (*a, *b));
                    }
                }
            }
            out
        };
        let score = |pairs: &BTreeMap<(usize, usize), (Row, Row)>| {
            SimpleScore::new(pairs.values().map(|(l, r)| r.1 - l.1).sum())
        };
        for round in 0..150 {
            let mut nl = l.clone();
            let mut nr = r.clone();
            let mut batch = StreamBatch::new();
            if round % 13 == 0 {
                for id in 0..8 {
                    batch.push(&right, TupleChange::retract(id)).unwrap();
                }
                nr.clear();
            } else {
                for step in 0..8 {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let x = seed >> 32;
                    let id = (x % 8) as usize;
                    let value = (!x.is_multiple_of(4))
                        .then_some(((x / 8 % 3) as i32, (x / 24 % 11) as i64));
                    let side = if step % 2 == 0 { &mut nl } else { &mut nr };
                    if let Some(v) = value {
                        side.insert(id, v);
                    } else {
                        side.remove(&id);
                    }
                    batch
                        .push(
                            if step % 2 == 0 { &left } else { &right },
                            TupleChange { id, value },
                        )
                        .unwrap();
                }
            }
            let mut tx = g.transaction();
            tx.apply_batch(batch).unwrap();
            let wanted = expected(&nl, &nr);
            assert_eq!(tx.score().unwrap(), score(&wanted));
            assert_eq!(
                tx.graph()
                    .unwrap()
                    .dataset_rows(&data)
                    .unwrap()
                    .iter()
                    .map(|(id, v)| (*id, *v))
                    .collect::<BTreeMap<_, _>>(),
                wanted
            );
            if round % 5 != 0 {
                tx.commit().unwrap();
                l = nl;
                r = nr;
            } else {
                drop(tx);
            }
            assert_eq!(g.score(), score(&expected(&l, &r)));
            assert_eq!(
                g.source_rows(&left)
                    .unwrap()
                    .iter()
                    .map(|(id, v)| (*id, *v))
                    .collect::<BTreeMap<_, _>>(),
                l
            );
            assert_eq!(
                g.source_rows(&right)
                    .unwrap()
                    .iter()
                    .map(|(id, v)| (*id, *v))
                    .collect::<BTreeMap<_, _>>(),
                r
            );
        }
        g.assert_consistent().unwrap();
    }
}
#[test]
fn sources_coalesce_before_mapping_and_a_self_join_settles_once() {
    let mapped = Cell::new(0);
    let scored = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let source = b.source::<usize, (usize, i64)>();
    let mapped_port = b
        .map(source.port(), |r| {
            mapped.set(mapped.get() + 1);
            assert!(r.1 >= 0);
            *r
        })
        .unwrap();
    let joined = b
        .join_batched(&mapped_port, &mapped_port, |_| (), |_| (), |l, r| l.0 < r.0)
        .unwrap();
    b.score("pairs", &joined, |(l, r)| {
        scored.set(scored.get() + 1);
        SimpleScore::new(l.1 + r.1)
    })
    .unwrap();
    let mut g = b.build();
    let mut batch = StreamBatch::new();
    batch.push(&source, TupleChange::put(0, (0, -99))).unwrap();
    for value in 0..100 {
        batch
            .push(&source, TupleChange::put(0, (0, value)))
            .unwrap();
    }
    batch.push(&source, TupleChange::put(1, (1, 1))).unwrap();
    let mut tx = g.transaction();
    tx.apply_batch(batch).unwrap();
    assert_eq!(tx.score_evaluations(), 1);
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(100));
    assert_eq!(mapped.get(), 2);
    assert_eq!(scored.get(), 1);
    let mut tx = g.transaction();
    let mut batch = StreamBatch::new();
    batch.push(&source, TupleChange::put(0, (0, 7))).unwrap();
    batch.push(&source, TupleChange::put(1, (1, 8))).unwrap();
    tx.apply_batch(batch).unwrap();
    assert_eq!(tx.score().unwrap(), SimpleScore::new(15));
    assert_eq!(tx.score_evaluations(), 1);
    drop(tx);
    assert_eq!(g.score(), SimpleScore::new(100));
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, (0, 5))).unwrap();
    assert_eq!(tx.score().unwrap(), SimpleScore::new(6));
    tx.commit().unwrap();
    g.assert_consistent().unwrap();
}
#[test]
fn foreign_batches_and_panics_poison_the_transaction_and_restore_all_nodes() {
    let fail = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let left = b.source::<usize, i64>();
    let right = b.source::<usize, i64>();
    let joined = b
        .join_batched(
            left.port(),
            right.port(),
            |_| {
                assert_ne!(fail.get(), 1);
            },
            |_| (),
            |_, _| {
                assert_ne!(fail.get(), 2);
                true
            },
        )
        .unwrap();
    let data = b.dataset(&joined).unwrap();
    b.score("pair", &joined, |(l, r)| {
        assert_ne!(fail.get(), 3);
        SimpleScore::new(l + r)
    })
    .unwrap();
    let mut g = b.build();
    let mut batch = StreamBatch::new();
    batch.push(&left, TupleChange::put(0, 1)).unwrap();
    batch.push(&right, TupleChange::put(0, 2)).unwrap();
    let mut tx = g.transaction();
    tx.apply_batch(batch).unwrap();
    tx.commit().unwrap();
    let before = g.dataset_rows(&data).unwrap().clone();
    for stage in 1..=3 {
        fail.set(stage);
        let mut batch = StreamBatch::new();
        batch.push(&left, TupleChange::put(0, 7)).unwrap();
        batch.push(&right, TupleChange::put(0, 9)).unwrap();
        let mut tx = g.transaction();
        assert!(catch_unwind(AssertUnwindSafe(|| tx.apply_batch(batch))).is_err());
        assert!(matches!(tx.score(), Err(GraphError::FailedTransaction)));
        assert!(matches!(tx.commit(), Err(GraphError::FailedTransaction)));
        assert_eq!(g.score(), SimpleScore::new(3));
        assert_eq!(*g.dataset_rows(&data).unwrap(), before);
    }
    fail.set(0);
    let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
    let fs = foreign.source::<usize, i64>();
    let mut mixed = StreamBatch::new();
    mixed.push(&left, TupleChange::put(0, 5)).unwrap();
    assert!(matches!(
        mixed.push(&fs, TupleChange::put(0, 6)),
        Err(GraphError::ForeignPort)
    ));
    assert_eq!(mixed.len(), 1);
    let mut bad = StreamBatch::new();
    bad.push(&fs, TupleChange::put(0, 6)).unwrap();
    let mut tx = g.transaction();
    assert!(matches!(tx.apply_batch(bad), Err(GraphError::ForeignPort)));
    drop(tx);
    assert_eq!(g.score(), SimpleScore::new(3));
    let mut tx = g.transaction();
    tx.apply_batch(mixed).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(7));
    g.assert_consistent().unwrap();
}
#[test]
fn checked_constraint_overflow_rolls_back_a_batch() {
    let mut b =
        StreamGraphBuilder::<SimpleScore>::new().with_arithmetic(ScoringArithmetic::Checked);
    let l = b.source::<usize, i64>();
    let r = b.source::<usize, i64>();
    let p = b
        .join_batched(l.port(), r.port(), |_| (), |_| (), |_, _| true)
        .unwrap();
    b.score("overflow", &p, |(l, _)| SimpleScore::new(*l))
        .unwrap();
    let mut g = b.build();
    let mut initial = StreamBatch::new();
    initial.push(&l, TupleChange::put(0, i64::MAX)).unwrap();
    initial.push(&r, TupleChange::put(0, 0)).unwrap();
    let mut tx = g.transaction();
    tx.apply_batch(initial).unwrap();
    tx.commit().unwrap();
    let mut changes = StreamBatch::new();
    changes.push(&l, TupleChange::put(1, 1)).unwrap();
    let mut tx = g.transaction();
    assert!(matches!(
        tx.apply_batch(changes),
        Err(GraphError::ScoreOverflow { .. })
    ));
    drop(tx);
    assert_eq!(g.score(), SimpleScore::new(i64::MAX));
    assert_eq!(g.source_rows(&l).unwrap().len(), 1);
}
#[test]
fn basic_composite_moves_batch_both_sides_and_restore_scopes() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1i64, 2, 3]).with_initial(1),
        Variable::new(vec![1, 2, 3]).with_initial(2),
    ])
    .unwrap();
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (usize, Option<i64>)>();
    let joined = b
        .join_batched(s.port(), s.port(), |_| (), |_| (), |a, b| a.0 < b.0)
        .unwrap();
    b.score("pair", &joined, |(a, b)| {
        calls.set(calls.get() + 1);
        SimpleScore::new(a.1.unwrap() + b.1.unwrap())
    })
    .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    assert_eq!(d.score(), SimpleScore::new(3));
    calls.set(0);
    let movement = Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(2),
        },
        Move::Change {
            variable: 1,
            value: Some(3),
        },
    ]);
    let trial = d.apply(&movement).unwrap();
    assert_eq!(trial.score(), SimpleScore::new(5));
    assert_eq!(trial.constraint_evaluations(), 1);
    assert_eq!(calls.get(), 1);
    drop(trial);
    assert_eq!(d.score(), SimpleScore::new(3));
    {
        let mut scope = d.edit_session().unwrap();
        scope.apply(&movement).unwrap();
        assert_eq!(scope.score(), SimpleScore::new(5));
    }
    assert_eq!(d.score(), SimpleScore::new(3));
    d.apply(&movement).unwrap().commit();
    d.assert_consistent().unwrap();
}
#[test]
fn list_and_mixed_moves_deliver_final_sources_in_one_round() {
    let lists = ListProblem::new(
        vec![10i32, 20],
        vec![ListEntity::new(vec![10, 20]), ListEntity::new(vec![])],
    )
    .unwrap();
    let movement = ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    };
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut b);
    let joined = b
        .join_batched(
            sources.entities.port(),
            sources.values.port(),
            |r| Some(r.0),
            |r| r.position.map(|p| p.entity),
            |a, b| a.1.contains(&b.value),
        )
        .unwrap();
    b.score("pair", &joined, |(a, b)| {
        SimpleScore::new(a.1.len() as i64 + i64::from(b.value))
    })
    .unwrap();
    let mut d =
        ListStreamScoreDirector::new(&lists, b.build(), sources, lists.initial_assignment())
            .unwrap();
    assert_eq!(d.score(), SimpleScore::new(34));
    let trial = d.apply(&movement).unwrap();
    assert_eq!(trial.score(), SimpleScore::new(32));
    assert_eq!(trial.constraint_evaluations(), 2);
    drop(trial);
    assert_eq!(d.score(), SimpleScore::new(34));
    d.apply(&movement).unwrap().commit();
    d.assert_consistent().unwrap();
    let basic = FiniteProblem::new(vec![
        Variable::new(vec![1i64, 2, 3]).with_initial(1),
        Variable::new(vec![1, 2, 3]).with_initial(2),
    ])
    .unwrap();
    let p = MixedProblem::new(basic, lists);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = MixedStreamSources::new(&mut b);
    let joined = b
        .join_batched(
            sources.basic.port(),
            sources.lists.entities.port(),
            |r| r.0,
            |r| r.0,
            |_, _| true,
        )
        .unwrap();
    b.score("pair", &joined, |(a, b)| {
        SimpleScore::new(a.1.unwrap() * b.1.len() as i64)
    })
    .unwrap();
    let mut d =
        MixedStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    assert_eq!(d.score(), SimpleScore::new(2));
    let movement = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(3),
        },
        lists: movement,
    };
    let trial = d.apply(&movement).unwrap();
    assert_eq!(trial.score(), SimpleScore::new(5));
    assert_eq!(trial.constraint_evaluations(), 2);
    drop(trial);
    assert_eq!(d.score(), SimpleScore::new(2));
    d.apply(&movement).unwrap().commit();
    d.assert_consistent().unwrap();
}

#[test]
fn saved_best_candidates_install_without_replaying_batched_join_scores() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1i64, 2, 3]).with_initial(1); 2]).unwrap();
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let s = b.source::<usize, (usize, Option<i64>)>();
    let joined = b
        .join_batched(s.port(), s.port(), |_| (), |_| (), |a, b| a.0 < b.0)
        .unwrap();
    b.score("pair", &joined, |(a, b)| {
        calls.set(calls.get() + 1);
        SimpleScore::new(a.1.unwrap() + b.1.unwrap())
    })
    .unwrap();
    let result = stream_local_search_from(
        &p,
        b.build(),
        s,
        p.initial_assignment(),
        LocalSearchConfig {
            max_evaluations: 40,
            max_steps: 5,
            selection: MoveSelection::BestAccepted,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.best.as_ref().unwrap().score, SimpleScore::new(6));
    assert_eq!(
        result.best.as_ref().unwrap().assignment.values(),
        &[Some(3), Some(3)]
    );
    assert_eq!(calls.get(), result.constraint_evaluations);
}
