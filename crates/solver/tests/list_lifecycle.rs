//! Original lifecycle plan validation, snapshot and failure regressions.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn problem() -> ListProblem<i32> {
    ListProblem::new(
        vec![0, 1, 2, 3, 4],
        vec![ListEntity::new(vec![0, 1, 2]), ListEntity::new(vec![3])],
    )
    .unwrap()
}
#[test]
fn callbacks_see_real_intermediate_states_and_flush_once() {
    let p = problem();
    let mut a = p.initial_assignment();
    // The boundary originally before2 adjusts after0 is removed. Value4 is initially absent.
    let plan = ListLifecyclePlan::gather(&p, &a, &[3, 0, 4], Some(pos(0, 2))).unwrap();
    assert_eq!(plan.assignment().lists(), &[vec![1, 3, 0, 4, 2], vec![]]);
    let mut flushes = 0;
    let mut stage_snapshots = vec![];
    let mut observer = |e: ListLifecycleEvent<'_, i32>| {
        if !e.undo
            && e.phase == ListChangePhase::After
            && matches!(e.change, ListLifecycleChange::Changed(_))
        {
            stage_snapshots.push(e.assignment.clone());
        }
        match e.change {
            ListLifecycleChange::Assigned(v) => assert_eq!(
                e.assignment.location(v).is_some(),
                e.phase == ListChangePhase::After
            ),
            ListLifecycleChange::Unassigned(v) => assert_eq!(
                e.assignment.location(v).is_some(),
                e.phase == ListChangePhase::Before
            ),
            ListLifecycleChange::Flush => {
                p.validate(e.assignment).unwrap();
                flushes += 1;
            }
            _ => {}
        }
    };
    plan.apply(&mut a, &mut observer).unwrap().commit();
    assert_eq!(
        stage_snapshots
            .iter()
            .map(|s| s.lists())
            .collect::<Vec<_>>(),
        vec![
            &[vec![0, 1, 2], vec![]][..],
            &[vec![1, 2], vec![]][..],
            &[vec![1, 3, 0, 4, 2], vec![]][..]
        ]
    );
    assert_eq!(flushes, 1);
    assert_eq!(a, *plan.assignment());
}
#[test]
fn every_callback_panic_restores_the_initial_assignment() {
    let p = problem();
    let original = p.initial_assignment();
    let plan = ListLifecyclePlan::gather(&p, &original, &[3, 0, 4], Some(pos(0, 2))).unwrap();
    let count = Cell::new(0);
    {
        let mut a = original.clone();
        plan.apply(&mut a, &mut |_| count.set(count.get() + 1))
            .unwrap()
            .rollback();
    }
    for fail_at in 0..count.get() {
        let mut a = original.clone();
        let calls = Cell::new(0);
        let mut observer = |_: ListLifecycleEvent<'_, i32>| {
            let n = calls.get();
            calls.set(n + 1);
            assert_ne!(n, fail_at, "listener panic");
        };
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                plan.apply(&mut a, &mut observer).unwrap().rollback();
            }))
            .is_err()
        );
        assert_eq!(a, original);
        assert_eq!(calls.get(), fail_at + 1);
    }
    let mut a = original.clone();
    let undo_calls = Cell::new(0);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut observer = |e: ListLifecycleEvent<'_, i32>| {
                if e.undo {
                    undo_calls.set(undo_calls.get() + 1)
                }
            };
            let _edit = plan.apply(&mut a, &mut observer).unwrap();
            panic!("unrelated panic");
        }))
        .is_err()
    );
    assert_eq!(a, original);
    assert_eq!(undo_calls.get(), 0);
}
#[test]
fn stale_plans_noops_and_model_errors_never_publish_changes() {
    let p = problem();
    let mut a = p.initial_assignment();
    let plan = ListLifecyclePlan::assign(&p, &a, &[4], pos(1, 1)).unwrap();
    let mut calls = 0;
    let mut observer = |_: ListLifecycleEvent<'_, i32>| calls += 1;
    ListLifecyclePlan::gather(&p, &a, &[], None)
        .unwrap()
        .apply(&mut a, &mut observer)
        .unwrap()
        .rollback();
    p.apply(
        &mut a,
        &ListMove::Unassign {
            entity: 1,
            start: 0,
            end: 1,
        },
    )
    .unwrap()
    .commit();
    assert!(matches!(
        plan.apply(&mut a, &mut observer),
        Err(StaleListPlan)
    ));
    assert_eq!(calls, 0);
    assert!(matches!(
        ListLifecyclePlan::gather(&p, &a, &[0, 0], None),
        Err(ListGatherError::DuplicateValue { index: 1 })
    ));
    let pinned = ListProblem::new(
        vec![0, 1],
        vec![ListEntity::new(vec![0, 1]).with_pinned_prefix(1)],
    )
    .unwrap();
    assert!(matches!(
        ListLifecyclePlan::gather(&pinned, &pinned.initial_assignment(), &[0], None),
        Err(ListGatherError::List(ListError::PinChanged { .. }))
    ));
    let dependent = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0]])
        .unwrap();
    assert!(matches!(
        ListLifecyclePlan::gather(
            &dependent,
            &dependent.initial_assignment(),
            &[1],
            Some(pos(0, 0))
        ),
        Err(ListGatherError::List(ListError::DependencyCycle { .. }))
    ));
}
#[test]
fn prepared_moves_work_in_both_scoring_backends() {
    let p = problem();
    let mut n = ListConstraintNetwork::new(2, 5);
    n.add_constraint(
        "position weight",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        |facts| {
            SimpleScore::new(
                facts
                    .iter()
                    .map(|fact| {
                        let ListFact::Entity(list) = fact else {
                            panic!()
                        };
                        list.iter()
                            .enumerate()
                            .map(|(i, v)| (i as i64 + 1) * i64::from(*v))
                            .sum::<i64>()
                    })
                    .sum(),
            )
        },
    )
    .unwrap();
    let initial = p.initial_assignment();
    let plan = ListLifecyclePlan::gather(&p, &initial, &[3, 0, 4], Some(pos(0, 2))).unwrap();
    let mut director = ListScoreDirector::new(&p, &n, initial.clone()).unwrap();
    let original_score = director.score();
    {
        let edit = director.apply(plan.movement()).unwrap();
        assert_eq!(edit.assignment(), plan.assignment());
        assert_eq!(edit.score(), SimpleScore::new(33));
    }
    assert_eq!(director.assignment(), &initial);
    assert_eq!(director.score(), original_score);
    director.apply(plan.movement()).unwrap().commit();
    director.assert_consistent().unwrap();
    assert_eq!(director.assignment(), plan.assignment());
    let mut builder = StreamGraphBuilder::new();
    let sources = ListStreamSources::<i32>::new(&mut builder);
    builder
        .score("position weight", sources.entities.port(), |(_, list)| {
            SimpleScore::new(
                list.iter()
                    .enumerate()
                    .map(|(i, v)| (i as i64 + 1) * i64::from(*v))
                    .sum(),
            )
        })
        .unwrap();
    let mut stream =
        ListStreamScoreDirector::new(&p, builder.build(), sources, initial.clone()).unwrap();
    {
        let edit = stream.apply(plan.movement()).unwrap();
        assert_eq!(edit.assignment(), plan.assignment());
        assert_eq!(edit.score(), SimpleScore::new(33));
    }
    assert_eq!(stream.assignment(), &initial);
    assert_eq!(stream.score(), original_score);
    stream.apply(plan.movement()).unwrap().commit();
    stream.assert_consistent().unwrap();
    assert_eq!(stream.assignment(), plan.assignment());
}

#[test]
fn assignment_and_compound_membership_changes_reverse_value_events() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![])],
    )
    .unwrap();
    let initial = p.initial_assignment();
    for plan in [
        ListLifecyclePlan::assign(&p, &initial, &[3, 2], pos(1, 0)).unwrap(),
        ListLifecyclePlan::prepare(
            &p,
            &initial,
            ListMove::Composite(vec![
                ListMove::Unassign {
                    entity: 0,
                    start: 0,
                    end: 1,
                },
                ListMove::Assign {
                    value: 2,
                    destination: pos(0, 0),
                },
            ]),
        )
        .unwrap(),
    ] {
        let mut a = initial.clone();
        let mut transitions = vec![];
        let mut observer = |e: ListLifecycleEvent<'_, i32>| match e.change {
            ListLifecycleChange::Assigned(v) => {
                assert_eq!(
                    e.assignment.location(v).is_some(),
                    e.phase == ListChangePhase::After
                );
                if e.phase == ListChangePhase::Before {
                    transitions.push((e.undo, true, *v));
                }
            }
            ListLifecycleChange::Unassigned(v) => {
                assert_eq!(
                    e.assignment.location(v).is_some(),
                    e.phase == ListChangePhase::Before
                );
                if e.phase == ListChangePhase::Before {
                    transitions.push((e.undo, false, *v));
                }
            }
            _ => {}
        };
        plan.apply(&mut a, &mut observer).unwrap().rollback();
        assert_eq!(a, initial);
        let forward: Vec<_> = transitions
            .iter()
            .filter(|e| !e.0)
            .map(|e| (e.1, e.2))
            .collect();
        let mut undone: Vec<_> = transitions
            .iter()
            .filter(|e| e.0)
            .map(|e| (!e.1, e.2))
            .collect();
        let mut sorted = forward.clone();
        sorted.sort();
        undone.sort();
        assert_eq!(sorted, undone);
        if !plan.returned_values().is_empty() {
            assert_eq!(forward, vec![(true, 3), (true, 2)]);
        } else {
            assert_eq!(forward, vec![(false, 0), (true, 2)]);
        }
    }
}
