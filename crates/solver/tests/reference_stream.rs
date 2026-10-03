//! Original reference propagation, transaction and search integration regressions.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[test]
fn indexed_updates_match_scan_after_every_generated_change() {
    let mut n = ReferenceNode::<usize, usize, Option<usize>, Option<i32>>::new(|v| *v);
    let mut left = BTreeMap::new();
    let mut right = BTreeMap::new();
    let mut observed = BTreeMap::new();
    let mut seed = 917_u64;
    for step in 0..2000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = (seed >> 32) as usize % 23;
        let output = if step % 3 != 0 {
            let value = if step % 7 == 0 {
                None
            } else {
                Some(if step % 11 == 0 {
                    None
                } else {
                    Some((seed >> 16) as usize % 23)
                })
            };
            if let Some(v) = value {
                left.insert(id, v);
            } else {
                left.remove(&id);
            }
            n.apply_left(TupleChange { id, value })
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            let value = if step % 5 == 0 {
                None
            } else {
                Some(if step % 13 == 0 { None } else { Some(step) })
            };
            if let Some(v) = value {
                right.insert(id, v);
            } else {
                right.remove(&id);
            }
            n.apply_right(TupleChange { id, value })
        };
        for event in output {
            if let Some(v) = event.value {
                observed.insert(event.id, v);
            } else {
                observed.remove(&event.id);
            }
        }
        let expected: BTreeMap<_, _> = left
            .iter()
            .map(|(&id, &key)| (id, (key, key.and_then(|k| right.get(&k).copied()))))
            .collect();
        assert_eq!(observed, expected, "step {step}");
        assert_eq!(
            n.rows()
                .map(|(&id, (l, r))| (id, (*l, r.copied())))
                .collect::<BTreeMap<_, _>>(),
            expected
        );
    }
}
#[test]
fn only_current_dependents_receive_target_changes_in_identity_order() {
    let mut n = ReferenceNode::<usize, usize, Option<usize>, i32>::new(|v| *v);
    for id in (0..1000).rev() {
        n.apply_left(TupleChange::put(id, Some(id % 100)));
    }
    assert!(n.apply_right(TupleChange::put(1000, 42)).is_empty());
    n.apply_left(TupleChange::put(3, Some(4)));
    n.apply_left(TupleChange::retract(103));
    n.apply_left(TupleChange::put(203, None));
    let events = n.apply_right(TupleChange::put(3, 9));
    assert_eq!(
        events.iter().map(|e| e.id).collect::<Vec<_>>(),
        (3..10).map(|i| i * 100 + 3).collect::<Vec<_>>()
    );
    assert!(events.iter().all(|e| e.value == Some((Some(3), Some(9)))));
    let events = n.apply_right(TupleChange::retract(3));
    assert_eq!(events.len(), 7);
    assert!(events.iter().all(|e| e.value == Some((Some(3), None))));
    assert!(n.apply_right(TupleChange::retract(3)).is_empty());
}
#[derive(Debug)]
struct Fragile(i32, Rc<Cell<bool>>);
impl Clone for Fragile {
    fn clone(&self) -> Self {
        assert!(!self.1.get(), "clone failure");
        Self(self.0, self.1.clone())
    }
}
#[test]
fn failed_mapping_and_payload_clone_leave_standalone_indexes_unchanged() {
    let fail = Rc::new(Cell::new(false));
    let flag = fail.clone();
    let mut n = ReferenceNode::<usize, usize, Option<usize>, Fragile>::new(move |v| {
        assert!(!flag.get());
        *v
    });
    n.apply_left(TupleChange::put(0, Some(1)));
    n.apply_right(TupleChange::put(1, Fragile(7, fail.clone())));
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || n.apply_left(TupleChange::put(0, Some(2)))
        ))
        .is_err()
    );
    assert!(
        catch_unwind(AssertUnwindSafe(
            || n.apply_right(TupleChange::put(1, Fragile(8, fail.clone())))
        ))
        .is_err()
    );
    fail.set(false);
    let rows: Vec<_> = n
        .rows()
        .map(|(&id, (v, r))| (id, *v, r.map(|r| r.0)))
        .collect();
    assert_eq!(rows, vec![(0, Some(1), Some(7))]);
    assert_eq!(
        n.apply_right(TupleChange::put(2, Fragile(9, fail.clone())))
            .len(),
        0
    );
    assert_eq!(
        n.apply_right(TupleChange::put(1, Fragile(10, fail))).len(),
        1
    );
}
#[test]
fn chained_references_propagate_missing_targets_and_rollback_without_callbacks() {
    let calls = Cell::new(0);
    let panic = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let entities = b.source::<usize, Option<usize>>();
    let groups = b.source::<usize, Option<usize>>();
    let owners = b.source::<usize, i64>();
    let group = b
        .follow_reference(entities.port(), groups.port(), |v| *v)
        .unwrap();
    let owner = b
        .follow_reference(&group, owners.port(), |(_, v)| v.flatten())
        .unwrap();
    let c = b
        .score("owner", &owner, |(_, v)| {
            calls.set(calls.get() + 1);
            assert!(!panic.get());
            SimpleScore::new(v.unwrap_or(0))
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&entities, TupleChange::put(0, Some(1))).unwrap();
    t.apply(&groups, TupleChange::put(1, Some(10))).unwrap();
    t.apply(&owners, TupleChange::put(10, 7)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(7));
    {
        let mut t = g.transaction();
        t.apply(&owners, TupleChange::retract(10)).unwrap();
        assert_eq!(t.score().unwrap(), SimpleScore::ZERO);
        let count = calls.get();
        drop(t);
        assert_eq!(calls.get(), count);
    }
    assert_eq!(g.score(), SimpleScore::new(7));
    panic.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut t = g.transaction();
            t.apply(&groups, TupleChange::put(1, None)).unwrap();
        }))
        .is_err()
    );
    panic.set(false);
    assert_eq!(g.score(), SimpleScore::new(7));
    g.assert_consistent().unwrap();
    let mut t = g.transaction();
    t.apply(&groups, TupleChange::put(1, Some(11))).unwrap();
    t.apply(&owners, TupleChange::put(11, 19)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.constraint_matches(&c).unwrap()[&0].0.1, Some(19));
    let mut t = g.transaction();
    t.apply(&owners, TupleChange::put(10, 100)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(19));
    g.assert_consistent().unwrap();
}
#[test]
fn same_source_references_and_payload_cycles_have_finite_one_hop_semantics() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, Option<usize>>();
    let r = b.follow_reference(s.port(), s.port(), |v| *v).unwrap();
    let c = b
        .score("count", &r, |(_, v)| {
            SimpleScore::new(i64::from(v.is_some()))
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for (id, value) in [
        (0, Some(0)),
        (1, Some(2)),
        (2, Some(1)),
        (3, None),
        (4, Some(9)),
    ] {
        t.apply(&s, TupleChange::put(id, value)).unwrap();
    }
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(3));
    assert_eq!(g.constraint_matches(&c).unwrap().len(), 5);
    g.assert_consistent().unwrap();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::retract(2)).unwrap();
    assert_eq!(t.score().unwrap(), SimpleScore::new(1));
    drop(t);
    assert_eq!(g.score(), SimpleScore::new(3));
    g.assert_consistent().unwrap();
}
fn graph() -> (
    StreamGraph<'static, SimpleScore>,
    PlanningStreamSource<usize>,
) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (usize, Option<usize>)>();
    let entities = b.filter(s.port(), |r| r.0 < 3).unwrap();
    let groups = b.filter(s.port(), |r| r.0 >= 3).unwrap();
    let r = b.follow_reference(&entities, &groups, |r| r.1).unwrap();
    b.score("owners", &r, |(e, g)| {
        SimpleScore::new(
            g.as_ref()
                .and_then(|r| r.1)
                .map_or(0, |owner| (owner as i64 - 5) * (e.0 as i64 + 1)),
        )
    })
    .unwrap();
    (b.build(), s)
}
#[test]
fn construction_and_saved_local_search_candidates_match_full_reference_scoring() {
    let p = FiniteProblem::new(
        (0..5)
            .map(|i| Variable::new(if i < 3 { vec![3, 4] } else { vec![5, 6, 7] }))
            .collect(),
    )
    .unwrap();
    let mut network = ConstraintNetwork::new(5);
    network
        .add_constraint("full", vec![0, 1, 2, 3, 4], |a| {
            SimpleScore::new(
                (0..3)
                    .map(|i| {
                        a[i].and_then(|&group| a[group])
                            .map_or(0, |&owner| (owner as i64 - 5) * (i as i64 + 1))
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
    let (g, s) = graph();
    let actual = greedy_stream_construction(&p, g, s, config).unwrap();
    assert_eq!(actual.assignment, expected.assignment);
    assert_eq!(actual.score, expected.score);
    for evaluations in [1, 2, 7, 40] {
        let config = LocalSearchConfig {
            max_evaluations: evaluations,
            assert_scores: true,
            ..Default::default()
        };
        let initial = Assignment::new(vec![Some(3), Some(4), Some(3), Some(5), Some(6)]);
        let mut expected = local_search_from(&p, &network, initial.clone(), config).unwrap();
        let (g, s) = graph();
        let mut actual = stream_local_search_from(&p, g, s, initial, config).unwrap();
        actual.constraint_evaluations = 0;
        expected.constraint_evaluations = 0;
        assert_eq!(actual, expected);
    }
}
#[test]
fn reference_builder_rejects_foreign_ports_before_adding_edges() {
    let mut a = StreamGraphBuilder::<SimpleScore>::new();
    let x = a.source::<usize, Option<usize>>();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let y = b.source::<usize, Option<usize>>();
    assert!(matches!(
        a.follow_reference(x.port(), y.port(), |v| *v),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        a.follow_reference(y.port(), x.port(), |v| *v),
        Err(GraphError::ForeignPort)
    ));
    let r = a.follow_reference(x.port(), x.port(), |v| *v).unwrap();
    a.score("valid", &r, |_| SimpleScore::ZERO).unwrap();
}
#[test]
fn composite_repoint_and_target_updates_restore_assignment_and_derived_score() {
    let p = FiniteProblem::new(
        vec![Variable::new(vec![3, 4]).with_initial(3); 3]
            .into_iter()
            .chain(vec![Variable::new(vec![5, 6, 7]).with_initial(5); 2])
            .collect(),
    )
    .unwrap();
    let (g, s) = graph();
    let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    let m = Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(4),
        },
        Move::Change {
            variable: 4,
            value: Some(7),
        },
        Move::Change {
            variable: 3,
            value: Some(6),
        },
    ]);
    {
        let trial = d.apply(&m).unwrap();
        assert_eq!(trial.score(), SimpleScore::new(7));
    }
    assert_eq!(d.assignment(), &p.initial_assignment());
    assert_eq!(d.score(), SimpleScore::ZERO);
    d.assert_consistent().unwrap();
    d.apply(&m).unwrap().commit();
    assert_eq!(d.score(), SimpleScore::new(7));
    assert!(
        d.apply(&Move::Change {
            variable: 4,
            value: Some(999)
        })
        .is_err()
    );
    assert_eq!(d.score(), SimpleScore::new(7));
    d.assert_consistent().unwrap();
}
#[test]
fn list_previous_reference_views_follow_swap_undo_and_commit() {
    let p = ListProblem::new(
        vec![0_usize, 1, 2],
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![2])],
    )
    .unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = ListStreamSources::new(&mut b);
    let prev = b
        .follow_reference(s.values.port(), s.values.port(), |v| v.previous)
        .unwrap();
    let c = b
        .score("previous value", &prev, |(_, v)| {
            SimpleScore::new(v.as_ref().map_or(0, |v| v.value as i64 + 1))
        })
        .unwrap();
    let mut d = ListStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    assert_eq!(d.score(), SimpleScore::new(1));
    let movement = ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 0,
        },
        right: ListPosition {
            entity: 1,
            index: 0,
        },
    };
    {
        let t = d.apply(&movement).unwrap();
        assert_eq!(t.score(), SimpleScore::new(3));
        let view = t.graph().constraint_matches(&c).unwrap();
        assert_eq!(view[&1].0.1.as_ref().unwrap().value, 2);
    }
    assert_eq!(d.score(), SimpleScore::new(1));
    d.assert_consistent().unwrap();
    d.apply(&movement).unwrap().commit();
    assert_eq!(d.score(), SimpleScore::new(3));
    d.assert_consistent().unwrap();
}
