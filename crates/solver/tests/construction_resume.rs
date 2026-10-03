//! Original resume, validation and observer checks; no upstream-method credit.
use timetuck::*;

type Event<A> = (A, SimpleScore, bool);
macro_rules! invoke {
    ($plain:path, $observed:path, $events:expr; $($arg:expr),*) => {
        if let Some(events) = $events {
            $observed($($arg,)* |u| events.push((u.assignment.clone(), u.score, u.complete)))
        } else {
            $plain($($arg),*)
        }.map_err(|e| e.to_string())
    };
}
fn basic_problem() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
        Variable::new(vec![1, 2]),
        Variable::new(vec![1, 2]).allowing_unassigned(),
        Variable::new(vec![1, 2]),
    ])
    .unwrap()
}
fn basic_initial() -> Assignment<i32> {
    Assignment::new(vec![Some(1), Some(2), None, None])
}
fn list_problem() -> ListProblem<i32> {
    ListProblem::new(
        vec![7, 8, 9, 10],
        vec![
            ListEntity::new(vec![7]).with_pinned(true),
            ListEntity::new(vec![8]).with_pinned_prefix(1),
        ],
    )
    .unwrap()
}
fn list_initial() -> ListAssignment<i32> {
    ListAssignment::new(vec![vec![7], vec![8, 9]])
}
fn basic(
    stream: bool,
    p: &FiniteProblem<i32>,
    a: Assignment<i32>,
    c: ConstructionConfig<'_>,
    events: Option<&mut Vec<Event<Assignment<i32>>>>,
) -> Result<ConstructionResult<i32, SimpleScore>, String> {
    if stream {
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<i32>)>();
        invoke!(greedy_stream_construction_from,greedy_stream_construction_from_with_observer,events; p,b.build(),s,a,c)
    } else {
        let n = ConstraintNetwork::new(p.variables().len());
        invoke!(greedy_construction_from,greedy_construction_from_with_observer,events; p,&n,a,c)
    }
}
fn lists(
    stream: bool,
    p: &ListProblem<i32>,
    a: ListAssignment<i32>,
    c: ConstructionConfig<'_>,
    events: Option<&mut Vec<Event<ListAssignment<i32>>>>,
) -> Result<ListConstructionResult<i32, SimpleScore>, String> {
    if stream {
        let mut b = StreamGraphBuilder::new();
        let s = ListStreamSources::new(&mut b);
        invoke!(greedy_list_stream_construction_from,greedy_list_stream_construction_from_with_observer,events; p,b.build(),s,a,c)
    } else {
        let n = ListConstraintNetwork::new(p.entities().len(), p.values().len());
        invoke!(greedy_list_construction_from,greedy_list_construction_from_with_observer,events; p,&n,a,c)
    }
}
fn mixed(
    stream: bool,
    p: &MixedProblem<i32, i32>,
    a: MixedAssignment<i32, i32>,
    c: MixedConstructionConfig<'_>,
    events: Option<&mut Vec<Event<MixedAssignment<i32, i32>>>>,
) -> Result<MixedConstructionResult<i32, i32, SimpleScore>, String> {
    if stream {
        let mut b = StreamGraphBuilder::new();
        let s = MixedStreamSources::new(&mut b);
        invoke!(greedy_mixed_stream_construction_from,greedy_mixed_stream_construction_from_with_observer,events; p,b.build(),s,a,c)
    } else {
        let n = MixedConstraintNetwork::new(
            p.basic().variables().len(),
            p.lists().entities().len(),
            p.lists().values().len(),
        );
        invoke!(greedy_mixed_construction_from,greedy_mixed_construction_from_with_observer,events; p,&n,a,c)
    }
}
#[test]
fn resumed_basic_preserves_supplied_pinned_and_optional_values() {
    let p = basic_problem();
    for stream in [false, true] {
        for max in 0..=5 {
            let c = ConstructionConfig {
                max_evaluations: max,
                assert_scores: true,
                ..Default::default()
            };
            let mut events = Vec::new();
            let r = basic(stream, &p, basic_initial(), c, Some(&mut events)).unwrap();
            assert_eq!(r, basic(stream, &p, basic_initial(), c, None).unwrap());
            assert_eq!(&r.assignment.values()[..3], &[Some(1), Some(2), None]);
            assert_eq!(r.evaluations, max.min(3));
            assert_eq!(r.assigned_variables, usize::from(max >= 2));
            assert_eq!(r.complete, max >= 2);
            if max == 0 {
                assert!(events.is_empty());
                assert_eq!(r.assignment, basic_initial());
            } else {
                assert_eq!(events[0].0, basic_initial());
                assert_eq!(events.last().unwrap().0, r.assignment);
            }
            let finished = basic(stream, &p, r.assignment, c_for_resume(), None).unwrap();
            assert!(finished.complete);
            assert_eq!(
                finished.assignment.values(),
                &[Some(1), Some(2), None, Some(1)]
            );
            let again = basic(
                stream,
                &p,
                finished.assignment.clone(),
                c_for_resume(),
                None,
            )
            .unwrap();
            assert_eq!(again.assignment, finished.assignment);
            assert_eq!(again.evaluations, 1);
            assert_eq!(again.assigned_variables, 0);
        }
    }
}
fn c_for_resume() -> ConstructionConfig<'static> {
    ConstructionConfig {
        assert_scores: true,
        ..Default::default()
    }
}
#[test]
fn resumed_lists_preserve_full_and_prefix_pins_and_prior_placements() {
    let p = list_problem();
    for stream in [false, true] {
        for max in 0..=5 {
            let c = ConstructionConfig {
                max_evaluations: max,
                ..c_for_resume()
            };
            let mut events = Vec::new();
            let r = lists(stream, &p, list_initial(), c, Some(&mut events)).unwrap();
            assert_eq!(r, lists(stream, &p, list_initial(), c, None).unwrap());
            assert_eq!(r.evaluations, max.min(3));
            assert_eq!(r.assigned_values, usize::from(max >= 2));
            assert_eq!(r.complete, max >= 2);
            if max == 0 {
                assert!(events.is_empty());
                assert_eq!(r.assignment, list_initial());
            } else {
                assert_eq!(events[0].0, list_initial());
                assert_eq!(events.last().unwrap().0, r.assignment);
            }
            let finished = lists(stream, &p, r.assignment, c_for_resume(), None).unwrap();
            assert!(finished.complete);
            assert_eq!(
                finished.assignment,
                ListAssignment::new(vec![vec![7], vec![8, 10, 9]])
            );
            let again = lists(
                stream,
                &p,
                finished.assignment.clone(),
                c_for_resume(),
                None,
            )
            .unwrap();
            assert_eq!(again.assignment, finished.assignment);
            assert_eq!(again.evaluations, 1);
            assert_eq!(again.assigned_values, 0);
        }
    }
}
#[test]
fn resumed_mixed_respects_component_order_and_completes_every_budget_prefix() {
    let p = MixedProblem::new(basic_problem(), list_problem());
    let initial = MixedAssignment::new(basic_initial(), list_initial());
    for stream in [false, true] {
        for order in [
            MixedConstructionOrder::BasicThenLists,
            MixedConstructionOrder::ListsThenBasic,
        ] {
            for max in 0..=6 {
                let c = MixedConstructionConfig {
                    construction: ConstructionConfig {
                        max_evaluations: max,
                        ..c_for_resume()
                    },
                    order,
                };
                let mut events = Vec::new();
                let r = mixed(stream, &p, initial.clone(), c, Some(&mut events)).unwrap();
                assert_eq!(r, mixed(stream, &p, initial.clone(), c, None).unwrap());
                assert_eq!(r.evaluations, max.min(5));
                if max == 0 {
                    assert!(events.is_empty());
                    assert_eq!(r.assignment, initial);
                } else {
                    assert_eq!(events[0].0, initial);
                    assert_eq!(events.last().unwrap().0, r.assignment);
                }
                if max == 2 {
                    assert_eq!(
                        r.assigned_basic_variables,
                        usize::from(order == MixedConstructionOrder::BasicThenLists)
                    );
                    assert_eq!(
                        r.assigned_list_values,
                        usize::from(order == MixedConstructionOrder::ListsThenBasic)
                    );
                }
                let finished = mixed(
                    stream,
                    &p,
                    r.assignment,
                    MixedConstructionConfig {
                        construction: c_for_resume(),
                        order,
                    },
                    None,
                )
                .unwrap();
                assert!(finished.complete);
                assert_eq!(
                    finished.assignment.basic().values(),
                    &[Some(1), Some(2), None, Some(1)]
                );
                assert_eq!(
                    finished.assignment.lists(),
                    &ListAssignment::new(vec![vec![7], vec![8, 10, 9]])
                );
            }
        }
    }
}
#[test]
fn resumed_validation_precedes_zero_budget_and_cancellation_without_observer_calls() {
    let token = CancellationToken::new();
    token.cancel();
    let invalid_basic = [
        Assignment::new(vec![]),
        Assignment::new(vec![Some(2), None, None, None]),
        Assignment::new(vec![Some(1), Some(9), None, None]),
    ];
    let invalid_lists = [
        ListAssignment::new(vec![]),
        ListAssignment::new(vec![vec![], vec![8, 9]]),
        ListAssignment::new(vec![vec![7], vec![9, 8]]),
        ListAssignment::new(vec![vec![7], vec![8, 8]]),
    ];
    let p = MixedProblem::new(basic_problem(), list_problem());
    for stream in [false, true] {
        for max in [0, 10] {
            for observed in [false, true] {
                let c = ConstructionConfig {
                    max_evaluations: max,
                    cancellation: Some(&token),
                    ..Default::default()
                };
                for a in &invalid_basic {
                    let mut events = Vec::new();
                    assert!(
                        basic(
                            stream,
                            p.basic(),
                            a.clone(),
                            c,
                            observed.then_some(&mut events)
                        )
                        .is_err()
                    );
                    assert!(events.is_empty());
                    let mut events = Vec::new();
                    assert!(
                        mixed(
                            stream,
                            &p,
                            MixedAssignment::new(a.clone(), list_initial()),
                            MixedConstructionConfig {
                                construction: c,
                                ..Default::default()
                            },
                            observed.then_some(&mut events)
                        )
                        .is_err()
                    );
                    assert!(events.is_empty());
                }
                for a in &invalid_lists {
                    let mut events = Vec::new();
                    assert!(
                        lists(
                            stream,
                            p.lists(),
                            a.clone(),
                            c,
                            observed.then_some(&mut events)
                        )
                        .is_err()
                    );
                    assert!(events.is_empty());
                    let mut events = Vec::new();
                    assert!(
                        mixed(
                            stream,
                            &p,
                            MixedAssignment::new(basic_initial(), a.clone()),
                            MixedConstructionConfig {
                                construction: c,
                                ..Default::default()
                            },
                            observed.then_some(&mut events)
                        )
                        .is_err()
                    );
                    assert!(events.is_empty());
                }
                let mut events = Vec::new();
                let r = basic(
                    stream,
                    p.basic(),
                    basic_initial(),
                    c,
                    observed.then_some(&mut events),
                )
                .unwrap();
                assert_eq!(r.assignment, basic_initial());
                assert_eq!(r.evaluations, 0);
                assert!(events.is_empty());
                let mut events = Vec::new();
                let r = lists(
                    stream,
                    p.lists(),
                    list_initial(),
                    c,
                    observed.then_some(&mut events),
                )
                .unwrap();
                assert_eq!(r.assignment, list_initial());
                assert_eq!(r.evaluations, 0);
                assert!(events.is_empty());
                let mut events = Vec::new();
                let initial = MixedAssignment::new(basic_initial(), list_initial());
                let r = mixed(
                    stream,
                    &p,
                    initial.clone(),
                    MixedConstructionConfig {
                        construction: c,
                        ..Default::default()
                    },
                    observed.then_some(&mut events),
                )
                .unwrap();
                assert_eq!(r.assignment, initial);
                assert_eq!(r.evaluations, 0);
                assert!(events.is_empty());
            }
        }
    }
}
