//! Native batched existence integration, not additional upstream-method credits.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;

#[test]
fn final_visibility_matches_scan_across_empty_sources_and_rollback() {
    type Row = (i32, i64);
    for mode in [ExistenceMode::Exists, ExistenceMode::NotExists] {
        for pruning in [false, true] {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            if pruning {
                b = b.with_activity_pruning();
            }
            let l = b.source::<usize, Row>();
            let r = b.source::<usize, Row>();
            let out = b
                .existence_batched(l.port(), r.port(), mode, |v| v.0, |v| v.0, |l, r| l.1 < r.1)
                .unwrap();
            let data = b.dataset(&out).unwrap();
            b.score("visible", &out, |v| SimpleScore::new(v.1)).unwrap();
            let mut g = b.build();
            let mut left = BTreeMap::new();
            let mut right = BTreeMap::new();
            let expected = |left: &BTreeMap<usize, Row>, right: &BTreeMap<usize, Row>| {
                left.iter()
                    .filter(|(_, l)| {
                        right.values().any(|r| l.0 == r.0 && l.1 < r.1)
                            == (mode == ExistenceMode::Exists)
                    })
                    .map(|(id, row)| (*id, *row))
                    .collect::<BTreeMap<_, _>>()
            };
            let mut seed = 91375u64;
            for round in 0..120 {
                let mut next_left = left.clone();
                let mut next_right = right.clone();
                let mut batch = StreamBatch::new();
                if round % 11 == 0 {
                    for id in 0..7 {
                        batch.push(&r, TupleChange::retract(id)).unwrap();
                    }
                    next_right.clear();
                } else {
                    for step in 0..8 {
                        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                        let x = seed >> 32;
                        let id = (x % 7) as usize;
                        let value = (!x.is_multiple_of(4))
                            .then_some(((x / 7 % 3) as i32, (x / 21 % 13) as i64));
                        let side = if step % 2 == 0 {
                            &mut next_left
                        } else {
                            &mut next_right
                        };
                        if let Some(v) = value {
                            side.insert(id, v);
                        } else {
                            side.remove(&id);
                        }
                        batch
                            .push(
                                if step % 2 == 0 { &l } else { &r },
                                TupleChange { id, value },
                            )
                            .unwrap();
                    }
                }
                let wanted = expected(&next_left, &next_right);
                let mut tx = g.transaction();
                tx.apply_batch(batch).unwrap();
                assert_eq!(
                    tx.score().unwrap(),
                    SimpleScore::new(wanted.values().map(|v| v.1).sum())
                );
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
                    left = next_left;
                    right = next_right;
                } else {
                    drop(tx);
                }
                assert_eq!(
                    g.dataset_rows(&data)
                        .unwrap()
                        .iter()
                        .map(|(id, v)| (*id, *v))
                        .collect::<BTreeMap<_, _>>(),
                    expected(&left, &right)
                );
                assert_eq!(
                    g.source_rows(&l)
                        .unwrap()
                        .iter()
                        .map(|(id, v)| (*id, *v))
                        .collect::<BTreeMap<_, _>>(),
                    left
                );
                assert_eq!(
                    g.source_rows(&r)
                        .unwrap()
                        .iter()
                        .map(|(id, v)| (*id, *v))
                        .collect::<BTreeMap<_, _>>(),
                    right
                );
            }
            g.assert_consistent().unwrap();
        }
    }
}

#[test]
fn witness_replacement_does_not_publish_a_transient_anti_join_row() {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let l = b.source::<usize, i64>();
    let r = b.source::<usize, i64>();
    let absent = b
        .existence_batched(
            l.port(),
            r.port(),
            ExistenceMode::NotExists,
            |_| (),
            |_| (),
            |l, r| l < r,
        )
        .unwrap();
    b.score("missing", &absent, |v| {
        calls.set(calls.get() + 1);
        SimpleScore::new(*v)
    })
    .unwrap();
    let mut g = b.build();
    let mut batch = StreamBatch::new();
    batch.push(&l, TupleChange::put(0, 3)).unwrap();
    batch.push(&r, TupleChange::put(0, 5)).unwrap();
    let mut tx = g.transaction();
    tx.apply_batch(batch).unwrap();
    tx.commit().unwrap();
    assert_eq!(calls.get(), 0);
    let mut batch = StreamBatch::new();
    batch.push(&r, TupleChange::retract(0)).unwrap();
    batch.push(&r, TupleChange::put(1, 6)).unwrap();
    let mut tx = g.transaction();
    tx.apply_batch(batch).unwrap();
    assert_eq!(tx.score_evaluations(), 0);
    tx.commit().unwrap();
    assert_eq!(calls.get(), 0);
    assert_eq!(g.score(), SimpleScore::ZERO);
    let mut tx = g.transaction();
    tx.apply(&r, TupleChange::retract(1)).unwrap();
    assert_eq!(tx.score().unwrap(), SimpleScore::new(3));
    drop(tx);
    assert_eq!(g.score(), SimpleScore::ZERO);
    let mut batch = StreamBatch::new();
    batch.push(&r, TupleChange::retract(1)).unwrap();
    let mut tx = g.transaction();
    tx.apply_batch(batch).unwrap();
    assert_eq!(tx.score().unwrap(), SimpleScore::new(3));
    tx.commit().unwrap();
    g.assert_consistent().unwrap();
}

#[test]
fn failed_round_restores_pending_state_and_solver_composites_settle_once() {
    let fail = Cell::new(false);
    let calls = Cell::new(0);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1i64, 2, 3]).with_initial(1),
        Variable::new(vec![1, 2, 3]).with_initial(2),
    ])
    .unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (usize, Option<i64>)>();
    let visible = b
        .existence_batched(
            s.port(),
            s.port(),
            ExistenceMode::Exists,
            |_| (),
            |_| (),
            |l, r| {
                assert!(!fail.get(), "predicate failure");
                l.0 != r.0 && l.1 < r.1
            },
        )
        .unwrap();
    b.score("has larger", &visible, |r| {
        calls.set(calls.get() + 1);
        SimpleScore::new(r.1.unwrap())
    })
    .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    assert_eq!(d.score(), SimpleScore::new(1));
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
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _trial = d.apply(&movement);
        }))
        .is_err()
    );
    fail.set(false);
    assert_eq!(d.score(), SimpleScore::new(1));
    let trial = d.apply(&movement).unwrap();
    assert_eq!(trial.score(), SimpleScore::new(2));
    assert_eq!(trial.constraint_evaluations(), 1);
    assert_eq!(calls.get(), 1);
    drop(trial);
    assert_eq!(d.score(), SimpleScore::new(1));
    {
        let mut scope = d.edit_session().unwrap();
        scope.apply(&movement).unwrap();
        assert_eq!(scope.score(), SimpleScore::new(2));
    }
    assert_eq!(d.score(), SimpleScore::new(1));
    d.apply(&movement).unwrap().commit();
    d.assert_consistent().unwrap();
}
