//! Original shared-supplier propagation and transactional solver regressions.
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn generated_membership_and_target_changes_match_scan_and_one_supplier_per_target() {
    let calls = RefCell::new(vec![]);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let l = b.source::<usize, (usize, usize)>();
    let r = b.source::<usize, i64>();
    let shared = b
        .follow_shared_reference(
            l.port(),
            r.port(),
            |v| v.1,
            |f, target| {
                calls.borrow_mut().push(f.0);
                target.copied()
            },
        )
        .unwrap();
    let c = b
        .score("shared", &shared, |(_, v)| SimpleScore::new(v.unwrap_or(0)))
        .unwrap();
    let mut g = b.build();
    let mut left = BTreeMap::new();
    let mut right = BTreeMap::new();
    let mut seed = 37_u64;
    for step in 0..1500 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = ((seed >> 32) % 31) as usize;
        calls.borrow_mut().clear();
        let mut t = g.transaction();
        if step % 3 != 0 {
            let key = (seed % 19) as usize;
            if step % 7 == 0 {
                left.remove(&id);
                t.apply(&l, TupleChange::retract(id)).unwrap();
            } else {
                left.insert(id, key);
                t.apply(&l, TupleChange::put(id, (id, key))).unwrap();
            }
        } else {
            let changed = if step % 5 == 0 {
                let existed = right.remove(&id).is_some();
                t.apply(&r, TupleChange::retract(id)).unwrap();
                existed
            } else {
                right.insert(id, step as i64);
                t.apply(&r, TupleChange::put(id, step as i64)).unwrap();
                true
            };
            let expected: Vec<_> = left
                .iter()
                .find(|(_, key)| **key == id)
                .filter(|_| changed)
                .map(|(&id, _)| id)
                .into_iter()
                .collect();
            assert_eq!(*calls.borrow(), expected, "supplier calls at {step}");
        }
        t.commit().unwrap();
        let expected: BTreeMap<_, _> = left
            .iter()
            .map(|(&id, &key)| (id, ((id, key), right.get(&key).copied())))
            .collect();
        let actual: BTreeMap<_, _> = g
            .constraint_matches(&c)
            .unwrap()
            .iter()
            .map(|(&id, (v, _))| (id, *v))
            .collect();
        assert_eq!(actual, expected, "state at {step}");
        assert_eq!(
            g.score(),
            SimpleScore::new(expected.values().map(|(_, v)| v.unwrap_or(0)).sum())
        );
    }
    g.assert_consistent().unwrap();
}
#[test]
fn representative_replacement_repointing_and_empty_groups() {
    let calls = RefCell::new(vec![]);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let l = b.source::<usize, (usize, usize)>();
    let r = b.source::<usize, i64>();
    let shared = b
        .follow_shared_reference(
            l.port(),
            r.port(),
            |v| v.1,
            |v, target| {
                calls.borrow_mut().push(v.0);
                target.copied().unwrap_or(-1)
            },
        )
        .unwrap();
    let c = b
        .score("shared", &shared, |(_, v)| SimpleScore::new(*v))
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for id in [9, 4, 1] {
        t.apply(&l, TupleChange::put(id, (id, 8))).unwrap();
    }
    t.apply(&r, TupleChange::put(8, 10)).unwrap();
    t.commit().unwrap();
    calls.borrow_mut().clear();
    let mut t = g.transaction();
    t.apply(&l, TupleChange::retract(1)).unwrap();
    assert_eq!(*calls.borrow(), vec![4]);
    t.apply(&l, TupleChange::put(4, (4, 7))).unwrap();
    t.commit().unwrap();
    let m = g.constraint_matches(&c).unwrap();
    assert_eq!(m[&4].0.1, -1);
    assert_eq!(m[&9].0.1, 10);
    let mut t = g.transaction();
    t.apply(&l, TupleChange::retract(9)).unwrap();
    calls.borrow_mut().clear();
    t.apply(&r, TupleChange::put(8, 20)).unwrap();
    assert!(calls.borrow().is_empty());
    t.commit().unwrap();
    g.assert_consistent().unwrap();
}
#[test]
fn rollback_and_callback_unwind_restore_shared_cache_without_supplier_replay() {
    let calls = Cell::new(0);
    let fail = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let l = b.source::<usize, usize>();
    let r = b.source::<usize, i64>();
    let v = b
        .follow_shared_reference(
            l.port(),
            r.port(),
            |v| *v,
            |_, r| {
                calls.set(calls.get() + 1);
                assert!(!fail.get());
                r.copied().unwrap_or(0)
            },
        )
        .unwrap();
    let c = b
        .score("shared", &v, |(_, v)| SimpleScore::new(*v))
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&l, TupleChange::put(0, 7)).unwrap();
    t.apply(&l, TupleChange::put(1, 7)).unwrap();
    t.apply(&r, TupleChange::put(7, 5)).unwrap();
    t.commit().unwrap();
    let old = g.constraint_matches(&c).unwrap().clone();
    {
        let mut t = g.transaction();
        t.apply(&r, TupleChange::put(7, 20)).unwrap();
        t.apply(&l, TupleChange::put(0, 8)).unwrap();
        assert_eq!(t.score().unwrap(), SimpleScore::new(20));
        let before = calls.get();
        drop(t);
        assert_eq!(calls.get(), before);
    }
    assert_eq!(g.constraint_matches(&c).unwrap(), &old);
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut t = g.transaction();
            t.apply(&r, TupleChange::put(7, 30)).unwrap();
        }))
        .is_err()
    );
    assert_eq!(g.constraint_matches(&c).unwrap(), &old);
    fail.set(false);
    g.assert_consistent().unwrap();
}
fn search_graph() -> (
    StreamGraph<'static, SimpleScore>,
    PlanningStreamSource<usize>,
) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (usize, Option<usize>)>();
    let followers = b.filter(s.port(), |v| v.0 < 4).unwrap();
    let targets = b.filter(s.port(), |v| v.0 >= 4).unwrap();
    let values = b
        .follow_shared_reference(
            &followers,
            &targets,
            |v| v.1.unwrap_or(usize::MAX),
            |_, t| t.and_then(|r| r.1),
        )
        .unwrap();
    b.score("weighted", &values, |(f, t)| {
        SimpleScore::new(t.map_or(0, |v| v as i64 * (f.0 as i64 + 1)))
    })
    .unwrap();
    (b.build(), s)
}
#[test]
fn construction_and_saved_candidates_match_independent_full_scoring() {
    let p = FiniteProblem::new(
        (0..6)
            .map(|i| Variable::new(if i < 4 { vec![4, 5] } else { vec![1, 2, 3] }))
            .collect(),
    )
    .unwrap();
    let mut network = ConstraintNetwork::new(6);
    network
        .add_constraint("full", (0..6).collect(), |a| {
            SimpleScore::new(
                (0..4)
                    .map(|i| {
                        a[i].and_then(|&j| a[j])
                            .map_or(0, |&v| v as i64 * (i as i64 + 1))
                    })
                    .sum(),
            )
        })
        .unwrap();
    let config = ConstructionConfig {
        assert_scores: true,
        ..Default::default()
    };
    let expected = greedy_construction(&p, &network, config).unwrap();
    let (g, s) = search_graph();
    let actual = greedy_stream_construction(&p, g, s, config).unwrap();
    assert_eq!(actual.assignment, expected.assignment);
    assert_eq!(actual.score, expected.score);
    for cap in [1, 2, 7, 40] {
        let config = LocalSearchConfig {
            max_evaluations: cap,
            assert_scores: true,
            ..Default::default()
        };
        let initial = Assignment::new(vec![Some(4), Some(5), Some(4), Some(5), Some(1), Some(2)]);
        let mut expected = local_search_from(&p, &network, initial.clone(), config).unwrap();
        let (g, s) = search_graph();
        let mut actual = stream_local_search_from(&p, g, s, initial, config).unwrap();
        actual.constraint_evaluations = 0;
        expected.constraint_evaluations = 0;
        assert_eq!(actual, expected);
    }
}
#[test]
fn same_source_payload_cycles_have_finite_shared_reference_semantics() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (usize, i64)>();
    let v = b
        .follow_shared_reference(s.port(), s.port(), |v| v.0, |_, t| t.map_or(0, |r| r.1))
        .unwrap();
    let c = b
        .score("shared", &v, |(_, v)| SimpleScore::new(*v))
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for (id, row) in [(0, (1, 5)), (1, (0, 7)), (2, (2, 11))] {
        t.apply(&s, TupleChange::put(id, row)).unwrap();
    }
    t.commit().unwrap();
    let m = g.constraint_matches(&c).unwrap();
    assert_eq!([m[&0].0.1, m[&1].0.1, m[&2].0.1], [7, 5, 11]);
    g.assert_consistent().unwrap();
}
#[test]
fn foreign_ports_are_rejected_before_any_composite_nodes_are_added() {
    let mut a = StreamGraphBuilder::<SimpleScore>::new();
    let x = a.source::<usize, usize>();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let y = b.source::<usize, usize>();
    assert!(matches!(
        a.follow_shared_reference(x.port(), y.port(), |v| *v, |_, t| t.copied()),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        a.follow_shared_reference(y.port(), x.port(), |v| *v, |_, t| t.copied()),
        Err(GraphError::ForeignPort)
    ));
    let v = a
        .follow_shared_reference(x.port(), x.port(), |v| *v, |_, t| t.copied())
        .unwrap();
    a.score("valid", &v, |_| SimpleScore::ZERO).unwrap();
}
