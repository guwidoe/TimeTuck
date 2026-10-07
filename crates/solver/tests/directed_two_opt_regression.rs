//! Original boundary, transaction and scoring regressions.
use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
#[test]
fn all_small_directed_cuts_preserve_membership_pins_and_rollback() {
    for n in 0..=8 {
        for prefix in 0..=n {
            let p = ListProblem::new(
                (0..n).collect(),
                vec![ListEntity::new((0..n).collect()).with_pinned_prefix(prefix)],
            )
            .unwrap();
            let mut a = p.initial_assignment();
            let original = a.clone();
            for first in prefix..=n {
                for second in prefix..=n {
                    let plan =
                        list_directed_two_opt_plan(&p, &a, pos(0, first), pos(0, second)).unwrap();
                    let mut sorted = plan.assignment().lists()[0].clone();
                    sorted.sort();
                    assert_eq!(sorted, (0..n).collect::<Vec<_>>());
                    assert_eq!(
                        &plan.assignment().lists()[0][..prefix],
                        &original.lists()[0][..prefix]
                    );
                    if first > second && prefix == 0 && n > 0 {
                        assert_eq!(plan.assignment().lists()[0][0], 0);
                    }
                    if first <= second {
                        let mut expected = original.lists()[0].clone();
                        expected[first..second].reverse();
                        assert_eq!(plan.assignment().lists()[0], expected);
                    }
                    {
                        let mut observer = |_: ListLifecycleEvent<'_, usize>| {};
                        let trial = plan.apply(&mut a, &mut observer).unwrap();
                        p.validate(trial.assignment()).unwrap();
                    }
                    assert_eq!(a, original);
                }
            }
        }
    }
}
#[test]
fn empty_tails_transfer_in_either_direction_and_both_empty_are_silent() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1, 2, 3]), ListEntity::new(vec![])],
    )
    .unwrap();
    let a = p.initial_assignment();
    for (x, y) in [(pos(0, 1), pos(1, 0)), (pos(1, 0), pos(0, 1))] {
        let plan = list_directed_two_opt_plan(&p, &a, x, y).unwrap();
        assert_eq!(plan.assignment().lists(), &[vec![1], vec![2, 3]]);
    }
    let plan = list_directed_two_opt_plan(&p, &a, pos(0, 3), pos(1, 0)).unwrap();
    let mut state = a.clone();
    let mut events = 0;
    plan.apply(&mut state, &mut |_: ListLifecycleEvent<'_, i32>| {
        events += 1
    })
    .unwrap()
    .commit();
    assert_eq!(state, a);
    assert_eq!(events, 0);
}
#[test]
fn invalid_cuts_full_pins_and_callback_panics_preserve_state() {
    let p = ListProblem::new(
        (0..5).collect(),
        vec![ListEntity::new((0..5).collect()).with_pinned_prefix(1)],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let original = a.clone();
    for cut in [pos(0, 0), pos(0, 6), pos(1, 0)] {
        assert!(list_directed_two_opt_plan(&p, &a, cut, pos(0, 2)).is_err());
    }
    let plan = list_directed_two_opt_plan(&p, &a, pos(0, 4), pos(0, 2)).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = plan.apply(&mut a, &mut |e: ListLifecycleEvent<'_, i32>| {
                if e.phase == ListChangePhase::After && !e.undo {
                    panic!("observer failure");
                }
            });
        }))
        .is_err()
    );
    assert_eq!(a, original);
    let pinned = ListProblem::new(
        (0..5).collect(),
        vec![ListEntity::new((0..5).collect()).with_pinned(true)],
    )
    .unwrap();
    assert!(matches!(
        list_directed_two_opt_plan(&pinned, &a, pos(0, 4), pos(0, 2)),
        Err(ListError::PinChanged { .. })
    ));
    let dependency = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0]])
        .unwrap();
    assert!(matches!(
        list_directed_two_opt_plan(
            &dependency,
            &dependency.initial_assignment(),
            pos(0, 0),
            pos(0, 2)
        ),
        Err(ListError::DependencyCycle { .. })
    ));
}
fn weighted(a: &ListAssignment<i32>) -> SimpleScore {
    SimpleScore::new(
        a.lists()[0]
            .iter()
            .enumerate()
            .map(|(i, &v)| (i as i64 + 1) * i64::from(v))
            .sum(),
    )
}
#[test]
fn wrapped_moves_update_both_scoring_backends_and_undo_cached_scores() {
    let p = ListProblem::new((0..6).collect(), vec![ListEntity::new((0..6).collect())]).unwrap();
    let mut n = ListConstraintNetwork::new(1, 6);
    n.add_constraint("positions", vec![ListScope::Entity(0)], |facts| {
        let ListFact::Entity(row) = &facts[0] else {
            unreachable!()
        };
        SimpleScore::new(
            row.iter()
                .enumerate()
                .map(|(i, &v)| (i as i64 + 1) * i64::from(v))
                .sum(),
        )
    })
    .unwrap();
    let mut builder = StreamGraphBuilder::new();
    let sources = ListStreamSources::new(&mut builder);
    builder
        .score("positions", sources.values.port(), |r| {
            SimpleScore::new(i64::from(r.value) * (r.position.unwrap().index as i64 + 1))
        })
        .unwrap();
    let mut scoped = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let mut stream =
        ListStreamScoreDirector::new(&p, builder.build(), sources, p.initial_assignment()).unwrap();
    for first in 1..=6 {
        for second in 0..first {
            let plan =
                list_directed_two_opt_plan(&p, scoped.assignment(), pos(0, first), pos(0, second))
                    .unwrap();
            let before = scoped.assignment().clone();
            {
                let a = scoped.apply(plan.movement()).unwrap();
                let b = stream.apply(plan.movement()).unwrap();
                assert_eq!(a.assignment(), b.assignment());
                assert_eq!(a.score(), weighted(a.assignment()));
                assert_eq!(a.score(), b.score());
            }
            assert_eq!(scoped.assignment(), &before);
            assert_eq!(stream.assignment(), &before);
            scoped.apply(plan.movement()).unwrap().commit();
            stream.apply(plan.movement()).unwrap().commit();
            scoped.assert_consistent().unwrap();
            stream.assert_consistent().unwrap();
        }
    }
}
