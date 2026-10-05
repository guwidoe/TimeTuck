//! Original precompute invalidation, transaction and solver integration checks.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[test]
fn occurrence_cache_ignores_variables_and_releases_removed_values() {
    let computed = Cell::new(0usize);
    let mut node = PrecomputeNode::<usize, (i32, i32), i32, Rc<i32>>::new(
        |r| r.0,
        |r| {
            computed.set(computed.get() + 1);
            Rc::new(r.0 * 3)
        },
    );
    let mut model = BTreeMap::new();
    let mut seed = 654321u64;
    let mut count = 0;
    for step in 0usize..600 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = (seed >> 32) as usize % 9;
        let input = (!step.is_multiple_of(5)).then_some(((seed % 4) as i32, step as i32));
        let old = model.get(&id).copied();
        let new_key = input.map(|r| r.0);
        let changes = node.apply(TupleChange { id, value: input });
        if old == new_key {
            assert!(changes.is_empty());
        } else {
            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0].id, id);
            assert_eq!(changes[0].value.as_deref().copied(), new_key.map(|k| k * 3));
            if new_key.is_some() {
                count += 1;
            }
        }
        if let Some(k) = new_key {
            model.insert(id, k);
        } else {
            model.remove(&id);
        }
        assert_eq!(computed.get(), count);
        assert_eq!(
            node.rows()
                .map(|(id, value)| (*id, **value))
                .collect::<BTreeMap<_, _>>(),
            model.iter().map(|(id, k)| (*id, k * 3)).collect()
        );
    }
    let first = node.apply(TupleChange::put(99, (7, 0)));
    let weak = Rc::downgrade(first[0].value.as_ref().unwrap());
    drop(first);
    assert!(node.apply(TupleChange::put(99, (7, 500))).is_empty());
    assert!(weak.upgrade().is_some());
    drop(node.apply(TupleChange::retract(99)));
    assert!(weak.upgrade().is_none());
}
#[test]
fn keyed_graph_matches_full_calculation_and_skips_fact_work_on_variable_changes() {
    type Row = (usize, i32, i64);
    let computations = Cell::new(0usize);
    let predicates = Cell::new(0usize);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Row>();
    let cached = b
        .precompute(
            source.port(),
            |r| (r.0, r.1),
            |r| {
                computations.set(computations.get() + 1);
                (r.0, r.1)
            },
        )
        .unwrap();
    let chosen = b
        .filter(&cached, |r| {
            predicates.set(predicates.get() + 1);
            r.1 % 2 == 0
        })
        .unwrap();
    let live = b
        .join_equal(&chosen, source.port(), |r| r.0, |r| r.0)
        .unwrap();
    let data = b.dataset(&live).unwrap();
    b.score("cost", &live, |((_, group), (_, _, value))| {
        SimpleScore::new(*group as i64 * value)
    })
    .unwrap();
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 13579u64;
    for step in 0usize..600 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = (seed >> 32) as usize % 8;
        let value = (!step.is_multiple_of(7)).then_some((
            id,
            ((seed >> 16) % 4) as i32,
            (seed % 11) as i64 - 5,
        ));
        let old = rows.get(&id).copied();
        let recompute = value.is_some_and(|r| old.map(|(_, group, _)| group) != Some(r.1));
        let mut expected = rows.clone();
        if let Some(v) = value {
            expected.insert(id, v);
        } else {
            expected.remove(&id);
        }
        let before = (computations.get(), predicates.get());
        let score = g.score();
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange { id, value }).unwrap();
        assert_eq!(
            (computations.get() - before.0, predicates.get() - before.1),
            (usize::from(recompute), usize::from(recompute))
        );
        let expected_rows: BTreeMap<_, _> = expected
            .iter()
            .filter(|(_, r)| r.1 % 2 == 0)
            .map(|(id, r)| ((*id, *id), ((r.0, r.1), *r)))
            .collect();
        assert_eq!(
            tx.graph()
                .unwrap()
                .dataset_rows(&data)
                .unwrap()
                .iter()
                .map(|(k, v)| (*k, *v))
                .collect::<BTreeMap<_, _>>(),
            expected_rows
        );
        assert_eq!(
            tx.score().unwrap(),
            SimpleScore::new(
                expected_rows
                    .values()
                    .map(|((_, k), (_, _, v))| *k as i64 * v)
                    .sum()
            )
        );
        if step.is_multiple_of(3) {
            drop(tx);
            assert_eq!(g.score(), score);
        } else {
            tx.commit().unwrap();
            rows = expected;
        }
    }
    g.assert_consistent().unwrap();
}
#[test]
fn cache_and_source_restore_after_key_compute_and_terminal_panics() {
    let failure = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i32>();
    let cached = b
        .precompute(
            source.port(),
            |v| {
                assert_ne!(failure.get(), 1);
                *v
            },
            |v| {
                assert_ne!(failure.get(), 2);
                Rc::new(*v)
            },
        )
        .unwrap();
    let data = b.dataset(&cached).unwrap();
    b.score("first", &cached, |v| SimpleScore::new(**v as i64))
        .unwrap();
    b.score("second", &cached, |v| {
        assert_ne!(failure.get(), 3);
        SimpleScore::new(**v as i64)
    })
    .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, 5)).unwrap();
    tx.commit().unwrap();
    let original = g
        .dataset_rows(&data)
        .unwrap()
        .iter()
        .next()
        .unwrap()
        .1
        .clone();
    for fail in 1..=3 {
        failure.set(fail);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let mut tx = g.transaction();
                tx.apply(&source, TupleChange::put(0, 8)).unwrap();
                tx.commit().unwrap();
            }))
            .is_err()
        );
        failure.set(0);
        assert_eq!(g.source_rows(&source).unwrap()[&0], 5);
        assert_eq!(g.score(), SimpleScore::new(10));
        assert!(Rc::ptr_eq(
            g.dataset_rows(&data).unwrap().iter().next().unwrap().1,
            &original
        ));
        g.assert_consistent().unwrap();
    }
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, 8)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(16));
}
#[test]
fn reusable_precompute_shares_callbacks_and_separates_definitions_parents_and_graphs() {
    let calls = Cell::new(0usize);
    let d = SharedPrecompute::new(
        |v: &i32| *v,
        |v: &i32| {
            calls.set(calls.get() + 1);
            *v
        },
    );
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i32>();
    let out = d.apply(&mut b, s.port()).unwrap();
    let again = d.clone().apply(&mut b, s.port()).unwrap();
    assert!(out.same_node(&again));
    assert!(!s.port().same_node(&out));
    let filtered = b.filter(s.port(), |_| true).unwrap();
    assert!(!out.same_node(&d.apply(&mut b, &filtered).unwrap()));
    let fresh = SharedPrecompute::new(|v: &i32| *v, |v: &i32| *v);
    assert!(!out.same_node(&fresh.apply(&mut b, s.port()).unwrap()));
    let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
    let fs = foreign.source::<usize, i32>();
    assert!(matches!(
        d.apply(&mut b, fs.port()),
        Err(GraphError::ForeignPort)
    ));
    assert!(!out.same_node(&d.apply(&mut foreign, fs.port()).unwrap()));
    b.score("a", &out, |v| SimpleScore::new(*v as i64)).unwrap();
    b.score("b", &again, |v| SimpleScore::new(*v as i64))
        .unwrap();
    drop(d);
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, 9)).unwrap();
    tx.commit().unwrap();
    // One computation for the source definition plus one for its filtered parent.
    assert_eq!(calls.get(), 2);
    assert_eq!(g.score(), SimpleScore::new(18));
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, 9)).unwrap();
    tx.commit().unwrap();
    assert_eq!(calls.get(), 2);
}
#[test]
fn basic_move_scoring_reuses_fact_computation_while_reading_current_assignments() {
    let calls = Cell::new(0usize);
    let weights = [3i64, 5];
    let problem = FiniteProblem::new(vec![
        Variable::new(vec![1i32, 2]),
        Variable::new(vec![1i32, 2]),
    ])
    .unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    let cached = b
        .precompute(
            source.port(),
            |r| r.0,
            |r| {
                calls.set(calls.get() + 1);
                (r.0, weights[r.0])
            },
        )
        .unwrap();
    let live = b
        .join_equal(&cached, source.port(), |r| r.0, |r| r.0)
        .unwrap();
    b.score("weighted", &live, |((_, weight), (_, value))| {
        SimpleScore::new(weight * value.unwrap_or(0) as i64)
    })
    .unwrap();
    let mut director =
        StreamScoreDirector::new(&problem, b.build(), source, problem.initial_assignment())
            .unwrap();
    assert_eq!(calls.get(), 2);
    director
        .apply(&Move::Change {
            variable: 0,
            value: Some(2),
        })
        .unwrap()
        .commit();
    director
        .apply(&Move::Change {
            variable: 1,
            value: Some(1),
        })
        .unwrap()
        .commit();
    assert_eq!(director.score(), SimpleScore::new(11));
    {
        let trial = director
            .apply(&Move::Change {
                variable: 1,
                value: Some(2),
            })
            .unwrap();
        assert_eq!(trial.score(), SimpleScore::new(16));
    }
    assert_eq!(director.score(), SimpleScore::new(11));
    assert_eq!(calls.get(), 2);
    director.assert_consistent().unwrap();
}
