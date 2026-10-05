//! Original tests; no upstream method credit.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
type Row = (u32, i64);
type Pairs = BTreeMap<(usize, usize), (Row, Row)>;
fn expected(rows: &BTreeMap<usize, Row>) -> Pairs {
    rows.iter()
        .flat_map(|(&i, &a)| {
            rows.iter()
                .filter_map(move |(&j, &b)| (i < j && a.0 == b.0).then_some(((i, j), (a, b))))
        })
        .collect()
}
fn random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed >> 32
}
#[test]
fn indexed_edits_match_full_pair_enumeration() {
    let mut n = UniquePairNode::<usize, Row, u32>::new(|r| r.0);
    let mut rows = BTreeMap::new();
    let mut observed = BTreeMap::new();
    let mut seed = 20261005;
    for step in 0..2000 {
        let id = (random(&mut seed) % 32) as usize;
        let value = (!random(&mut seed).is_multiple_of(4)).then(|| {
            (
                (random(&mut seed) % 5) as u32,
                (random(&mut seed) % 20) as i64,
            )
        });
        if let Some(row) = value {
            rows.insert(id, row);
        } else {
            rows.remove(&id);
        }
        for change in n.apply(TupleChange { id, value }) {
            assert!(change.id.0 < change.id.1);
            if let Some(row) = change.value {
                observed.insert(change.id, row);
            } else {
                assert!(observed.remove(&change.id).is_some());
            }
        }
        let want = expected(&rows);
        assert_eq!(observed, want, "step {step}");
        assert_eq!(
            n.rows()
                .map(|((i, j), (a, b))| ((*i, *j), (*a, *b)))
                .collect::<Pairs>(),
            want
        );
        assert_eq!(n.input_len(), rows.len());
    }
    for id in 0..32 {
        n.apply(TupleChange::retract(id));
    }
    assert_eq!(n.input_len(), 0);
    assert_eq!(n.key_count(), 0);
}
#[test]
fn updates_touch_only_matching_peers_and_retractions_never_project() {
    let calls = Cell::new(0);
    let mut n = UniquePairNode::<usize, Row, u32>::new(|r| {
        calls.set(calls.get() + 1);
        r.0
    });
    for id in 0..1000 {
        assert!(n.apply(TupleChange::put(id, (id as u32, 1))).is_empty());
    }
    assert_eq!(calls.get(), 1000);
    assert_eq!(
        n.apply(TupleChange::put(1001, (5, 9))),
        vec![TupleChange::put((5, 1001), ((5, 1), (5, 9)))]
    );
    assert_eq!(
        n.apply(TupleChange::put(5, (5, 2))),
        vec![TupleChange::put((5, 1001), ((5, 2), (5, 9)))]
    );
    assert_eq!(
        n.apply(TupleChange::retract(5)),
        vec![TupleChange::retract((5, 1001))]
    );
    assert!(n.apply(TupleChange::retract(5)).is_empty());
    assert_eq!(calls.get(), 1002);
}
#[test]
fn equal_payloads_keep_identity_and_key_panics_preserve_state() {
    let fail = Cell::new(false);
    let mut n = UniquePairNode::<usize, i32, ()>::new(|_| {
        assert!(!fail.get(), "injected");
    });
    n.apply(TupleChange::put(9, 7));
    assert_eq!(
        n.apply(TupleChange::put(1, 7)),
        vec![TupleChange::put((1, 9), (7, 7))]
    );
    fail.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| n.apply(TupleChange::put(1, 8)))).is_err());
    assert_eq!(
        n.rows()
            .map(|((i, j), (a, b))| ((*i, *j), (*a, *b)))
            .collect::<Vec<_>>(),
        vec![((1, 9), (7, 7))]
    );
    assert_eq!(
        n.apply(TupleChange::retract(1)),
        vec![TupleChange::retract((1, 9))]
    );
    fail.set(false);
    n.apply(TupleChange::put(1, 8));
    assert_eq!(
        n.rows().map(|(_, (a, b))| (*a, *b)).collect::<Vec<_>>(),
        vec![(8, 7)]
    );
}
#[test]
fn graph_trials_roll_back_pairs_filters_and_shared_scoring_branches() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Row>();
    let pairs = b.unique_pairs_equal(source.port(), |r| r.0).unwrap();
    let selected = b.filter(&pairs, |(a, b)| a.1 < b.1).unwrap();
    let dataset = b.dataset(&pairs).unwrap();
    b.score("all", &pairs, |_| SimpleScore::new(-1)).unwrap();
    b.score("ordered", &selected, |_| SimpleScore::new(-2))
        .unwrap();
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261006;
    for step in 0..500 {
        let mut next = rows.clone();
        let mut tx = g.transaction();
        for _ in 0..3 {
            let id = (random(&mut seed) % 15) as usize;
            let value = (!random(&mut seed).is_multiple_of(4)).then(|| {
                (
                    (random(&mut seed) % 3) as u32,
                    (random(&mut seed) % 9) as i64,
                )
            });
            if let Some(row) = value {
                next.insert(id, row);
            } else {
                next.remove(&id);
            }
            tx.apply(&source, TupleChange { id, value }).unwrap();
        }
        let want = expected(&next);
        let actual: Pairs = tx
            .graph()
            .unwrap()
            .dataset_rows(&dataset)
            .unwrap()
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect();
        assert_eq!(actual, want);
        let penalty = want.len() + 2 * want.values().filter(|(a, b)| a.1 < b.1).count();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(-(penalty as i64)));
        if step % 3 == 0 {
            tx.commit().unwrap();
            rows = next;
        } else {
            drop(tx);
        }
        let actual: Pairs = g
            .dataset_rows(&dataset)
            .unwrap()
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect();
        assert_eq!(actual, expected(&rows));
        if step % 50 == 0 {
            g.assert_consistent().unwrap();
        }
    }
}
#[test]
fn downstream_panic_restores_source_and_pair_index_and_foreign_port_is_rejected() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
    let wrong = foreign.source::<usize, i32>();
    assert!(matches!(
        b.unique_pairs(wrong.port()),
        Err(GraphError::ForeignPort)
    ));
    let source = b.source::<usize, i32>();
    let pairs = b.unique_pairs(source.port()).unwrap();
    let fail = Cell::new(false);
    b.score("pairs", &pairs, |_| {
        assert!(!fail.get(), "injected score");
        SimpleScore::new(-1)
    })
    .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, 1)).unwrap();
    tx.commit().unwrap();
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::put(1, 2)).unwrap();
        }))
        .is_err()
    );
    assert_eq!(g.source_rows(&source).unwrap().len(), 1);
    assert_eq!(g.score(), SimpleScore::ZERO);
    fail.set(false);
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(1, 2)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(-1));
    g.assert_consistent().unwrap();
}
#[test]
fn planning_composite_trials_update_pairs_and_restore_assignments() {
    let p = FiniteProblem::new(
        (0..4)
            .map(|_| Variable::new(vec![0, 1, 2]).with_initial(0))
            .collect(),
    )
    .unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    let assigned = b.filter(source.port(), |r| r.1.is_some()).unwrap();
    let pairs = b.unique_pairs_equal(&assigned, |r| r.1).unwrap();
    b.score("conflicts", &pairs, |_| SimpleScore::new(-1))
        .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    assert_eq!(d.score(), SimpleScore::new(-6));
    let movement = Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(1),
        },
        Move::Change {
            variable: 1,
            value: Some(2),
        },
    ]);
    {
        let t = d.apply(&movement).unwrap();
        assert_eq!(t.score(), SimpleScore::new(-1));
    }
    assert_eq!(d.score(), SimpleScore::new(-6));
    assert_eq!(d.assignment().values(), &[Some(0); 4]);
    d.apply(&movement).unwrap().commit();
    assert_eq!(d.score(), SimpleScore::new(-1));
    d.assert_consistent().unwrap();
}
