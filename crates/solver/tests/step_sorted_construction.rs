//! Original dynamic comparator, retry, reset and failure-publication tests.
use std::{
    cell::Cell,
    cmp::Ordering,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn same(_: usize, _: usize) -> Ordering {
    Ordering::Equal
}
fn same_value(_: &i32, _: &i32) -> Ordering {
    Ordering::Equal
}
#[test]
fn basic_step_factories_change_actual_scored_choices_in_all_placements() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 3]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    for placement in [
        ConstructionPlacement::Sequential,
        ConstructionPlacement::ValueQueue,
        ConstructionPlacement::Pooled,
    ] {
        let calls = Cell::new(0);
        let mut provider = SortedBasicConstruction::new(
            &p,
            placement,
            OptionalConstruction::Preserve,
            same,
            same_value,
        )
        .with_step_sorting(|_: &FiniteProblem<i32>, a: &Assignment<i32>| {
            let step = a.values().iter().flatten().count();
            calls.set(calls.get() + 1);
            let entity = [2, 0, 1][step];
            let value = 2 - step as i32;
            (
                move |a: usize, b: usize| (a != entity).cmp(&(b != entity)),
                move |a: &i32, b: &i32| (a != &value).cmp(&(b != &value)),
            )
        });
        let r = construction_from_with_provider(
            &p,
            &n,
            p.initial_assignment(),
            ProviderConstructionConfig {
                assert_scores: true,
                ..Default::default()
            },
            &mut provider,
        )
        .unwrap();
        let expected = match placement {
            ConstructionPlacement::Sequential => [2, 1, 0],
            ConstructionPlacement::ValueQueue => [1, 2, 0],
            ConstructionPlacement::Pooled => [1, 0, 2],
        };
        assert_eq!(r.assignment.values(), expected.map(Some));
        assert_eq!(calls.get(), 3);
    }
}
#[test]
fn list_step_factories_see_current_lengths_and_sort_destinations_and_pool_values() {
    let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![]); 3]).unwrap();
    for placement in [
        ConstructionPlacement::Sequential,
        ConstructionPlacement::ValueQueue,
        ConstructionPlacement::Pooled,
    ] {
        let calls = Cell::new(0);
        let mut provider = SortedListConstruction::new(
            &p,
            placement,
            OptionalConstruction::Preserve,
            same,
            same_value,
        )
        .with_step_sorting(|_: &ListProblem<i32>, a: &ListAssignment<i32>| {
            let count = a.lists().iter().map(Vec::len).sum::<usize>();
            calls.set(calls.get() + 1);
            let target = [2, 0, 1][count];
            let value = 2 - count as i32;
            (
                move |a: usize, b: usize| (a != target).cmp(&(b != target)),
                move |a: &i32, b: &i32| (a != &value).cmp(&(b != &value)),
            )
        });
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let sources = ListStreamSources::new(&mut b);
        b.score("zero", sources.entities.port(), |_| SimpleScore::ZERO)
            .unwrap();
        let r = list_stream_construction_from_with_provider(
            &p,
            b.build(),
            sources,
            p.initial_assignment(),
            ProviderConstructionConfig {
                assert_scores: true,
                ..Default::default()
            },
            &mut provider,
        )
        .unwrap();
        let expected = if placement == ConstructionPlacement::Pooled {
            [1, 0, 2]
        } else {
            [1, 2, 0]
        };
        assert_eq!(r.assignment.lists(), expected.map(|v| vec![v]));
        assert_eq!(calls.get(), 3);
    }
}
#[test]
fn retries_reuse_step_cache_reset_rebuilds_it_and_panics_do_not_advance_keys() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 2]).unwrap();
    let calls = Cell::new(0);
    let panic = Cell::new(false);
    let reverse = Cell::new(true);
    let mut provider = SortedBasicConstruction::new(
        &p,
        ConstructionPlacement::Sequential,
        OptionalConstruction::Preserve,
        same,
        same_value,
    )
    .with_step_sorting(|_: &FiniteProblem<i32>, _: &Assignment<i32>| {
        calls.set(calls.get() + 1);
        let descending = reverse.get();
        let panic = &panic;
        (
            same as fn(usize, usize) -> Ordering,
            move |a: &i32, b: &i32| {
                assert!(!panic.get(), "comparator failure");
                if descending {
                    b.cmp(a)
                } else {
                    Ordering::Equal
                }
            },
        )
    });
    let a = p.initial_assignment();
    let first = provider.moves(&p, &a, 0).next().unwrap();
    assert_eq!(
        first,
        Move::Change {
            variable: 0,
            value: Some(2)
        }
    );
    reverse.set(false);
    assert_eq!(provider.moves(&p, &a, 0).next(), Some(first));
    assert_eq!(calls.get(), 1);
    panic.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| provider.moves(&p, &a, 1).next())).is_err());
    panic.set(false);
    // Equal comparator starts from original domain encounter order, not last step's reverse.
    assert_eq!(
        provider.moves(&p, &a, 1).next(),
        Some(Move::Change {
            variable: 1,
            value: Some(0)
        })
    );
    assert_eq!(calls.get(), 3);
    provider.reset();
    assert_eq!(
        provider.moves(&p, &a, 0).next(),
        Some(Move::Change {
            variable: 0,
            value: Some(0)
        })
    );
    assert_eq!(calls.get(), 4);
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    provider.reset();
    let r = construction_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        ProviderConstructionConfig {
            max_move_evaluations: Some(1),
            interruption: ConstructionInterruption::DiscardIncompleteStep,
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap();
    let before = calls.get();
    assert_eq!(r.completed_steps, 0);
    assert_eq!(r.assignment, a);
    let r = construction_from_with_provider(
        &p,
        &n,
        r.assignment,
        ProviderConstructionConfig {
            max_steps: Some(1),
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap();
    assert_eq!(r.assignment.values(), &[Some(0), None]);
    assert_eq!(calls.get(), before);
}
