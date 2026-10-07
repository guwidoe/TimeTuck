//! Original diagnostics and error-ordering checks across scoring backends.
use std::cell::Cell;
use timetuck::*;

fn basic() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        Variable::new(vec![1]),
        Variable::new(vec![1])
            .allowing_unassigned()
            .with_pinned(true),
        Variable::new(vec![1]).with_pinned(true),
        Variable::new(vec![1]).with_pinned(true),
    ])
    .unwrap()
}
#[test]
fn basic_reports_first_blocked_pin_before_scoring_or_opening_provider() {
    let p = basic();
    for stream in [false, true] {
        for budget in [0, 10] {
            let calls = Cell::new(0);
            let mut provider =
                |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| -> Vec<Move<i32>> {
                    panic!("invalid start opened provider")
                };
            let cfg = LocalSearchConfig {
                max_evaluations: budget,
                ..Default::default()
            };
            let error = if stream {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let s = b.source::<usize, (usize, Option<i32>)>();
                b.score("count", s.port(), |_| {
                    calls.set(calls.get() + 1);
                    SimpleScore::ZERO
                })
                .unwrap();
                stream_local_search_from_with_provider(
                    &p,
                    b.build(),
                    s,
                    p.initial_assignment(),
                    cfg,
                    &mut provider,
                )
            } else {
                let mut n = ConstraintNetwork::new(4);
                n.add_constraint("count", vec![], |_| {
                    calls.set(calls.get() + 1);
                    SimpleScore::ZERO
                })
                .unwrap();
                local_search_from_with_provider(&p, &n, p.initial_assignment(), cfg, &mut provider)
            }
            .unwrap_err();
            assert!(matches!(
                error,
                LocalSearchError::PinnedUnassignedVariable { variable: 2 }
            ));
            assert!(error.to_string().contains("pinned required variable 2"));
            assert_eq!(calls.get(), 0);
        }
    }
}
#[test]
fn mixed_reports_basic_pin_even_when_list_membership_is_incomplete() {
    let p = MixedProblem::new(
        basic(),
        ListProblem::new(vec![9], vec![ListEntity::new(vec![])]).unwrap(),
    );
    for stream in [false, true] {
        for budget in [0, 10] {
            let calls = Cell::new(0);
            let mut provider = |_: &MixedProblem<i32, i32>,
                                _: &MixedAssignment<i32, i32>,
                                _: u64|
             -> Vec<MixedMove<i32, i32>> {
                panic!("invalid start opened provider")
            };
            let cfg = MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_evaluations: budget,
                    ..Default::default()
                },
                ..Default::default()
            };
            let error = if stream {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let s = MixedStreamSources::new(&mut b);
                b.score("count", s.basic.port(), |_| {
                    calls.set(calls.get() + 1);
                    SimpleScore::ZERO
                })
                .unwrap();
                mixed_stream_local_search_from_with_provider(
                    &p,
                    b.build(),
                    s,
                    p.initial_assignment(),
                    cfg,
                    &mut provider,
                )
            } else {
                let mut n = MixedConstraintNetwork::new(4, 1, 1);
                n.add_constraint("count", vec![], |_| {
                    calls.set(calls.get() + 1);
                    SimpleScore::ZERO
                })
                .unwrap();
                mixed_local_search_from_with_provider(
                    &p,
                    &n,
                    p.initial_assignment(),
                    cfg,
                    &mut provider,
                )
            }
            .unwrap_err();
            assert!(matches!(
                error,
                MixedLocalSearchError::PinnedUnassignedVariable { variable: 2 }
            ));
            assert!(error.to_string().contains("basic variable 2"));
            assert_eq!(calls.get(), 0);
        }
    }
}
#[test]
fn model_errors_keep_precedence_and_optional_or_initialized_pins_are_valid() {
    let p = basic();
    let n = ConstraintNetwork::<_, SimpleScore>::new(4);
    let config = LocalSearchConfig {
        max_evaluations: 0,
        ..Default::default()
    };
    for a in [
        Assignment::new(vec![None]),
        Assignment::new(vec![Some(99), None, None, None]),
    ] {
        assert!(matches!(
            local_search_from(&p, &n, a, config),
            Err(LocalSearchError::Scoring(ScoringError::Model(_)))
        ));
    }
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1]).with_initial(1).with_pinned(true),
        Variable::new(vec![1])
            .allowing_unassigned()
            .with_pinned(true),
    ])
    .unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    assert!(local_search_from(&p, &n, p.initial_assignment(), config).is_ok());
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1]),
        Variable::new(vec![1])
            .allowing_unassigned()
            .with_pinned(true),
    ])
    .unwrap();
    assert!(matches!(
        local_search_from(&p, &n, p.initial_assignment(), config),
        Err(LocalSearchError::IncompleteInitialAssignment)
    ));
}
