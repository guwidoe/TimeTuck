//! Original integration coverage for construction providers in complete plans.
use std::cell::Cell;
use timetuck::*;
fn basic_problem() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap()
}
fn list_problem() -> ListProblem<i32> {
    ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![])]).unwrap()
}
fn mixed_problem() -> MixedProblem<i32, i32> {
    MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1, 2])]).unwrap(),
        ListProblem::new(vec![1], vec![ListEntity::new(vec![])]).unwrap(),
    )
}
fn basic_count(a: &Assignment<i32>) -> usize {
    a.values().iter().flatten().count()
}
fn list_count(a: &ListAssignment<i32>) -> usize {
    a.lists().iter().map(Vec::len).sum()
}
fn mixed_count(a: &MixedAssignment<i32, i32>) -> usize {
    basic_count(a.basic()) + list_count(a.lists())
}
fn basic_move(a: &Assignment<i32>) -> Move<i32> {
    Move::Change {
        variable: a.values().iter().position(Option::is_none).unwrap(),
        value: Some(1),
    }
}
fn list_move(a: &ListAssignment<i32>) -> ListMove<i32> {
    ListMove::Assign {
        value: if a.location(&1).is_none() { 1 } else { 2 },
        destination: ListPosition {
            entity: 0,
            index: a.lists()[0].len(),
        },
    }
}
fn mixed_move(a: &MixedAssignment<i32, i32>) -> MixedMove<i32, i32> {
    if a.basic().values()[0].is_none() {
        MixedMove {
            basic: basic_move(a.basic()),
            lists: ListMove::Composite(vec![]),
        }
    } else {
        MixedMove {
            basic: Move::Composite(vec![]),
            lists: list_move(a.lists()),
        }
    }
}
macro_rules! exercise {
    ($P:ty,$A:ty,$M:ty,$p:expr,$objective:expr,$solve:ident,$Plan:ident,$construction:expr,$local:expr,$exact:expr,$movement:ident,$count:ident) => {{
        let p = $p;
        let construction = $construction;
        let local = $local;
        for mode in 0..9 {
            let token = CancellationToken::new();
            let condition = SearchTermination::Condition(SearchLimit::Steps(1));
            let mut seen = vec![];
            let mut provider = |_: &$P, a: &$A, step| {
                seen.push((step, $count(a)));
                if mode == 5 {
                    vec![]
                } else {
                    vec![$movement(a)]
                }
            };
            let mut second_seen = vec![];
            let mut second = |_: &$P, a: &$A, step| {
                second_seen.push((step, $count(a)));
                vec![$movement(a)]
            };
            let mut best = vec![];
            let mut initialized = vec![];
            let mut events = |event: SolverEvent<'_, $A, SimpleScore>| match event {
                SolverEvent::Best(u) => {
                    best.push((u.phase, $count(u.assignment), u.complete));
                    if mode == 6 && $count(u.assignment) == 1 {
                        token.cancel();
                    }
                }
                SolverEvent::FirstInitialized {
                    initialization,
                    complete,
                    ..
                } => initialized.push((
                    initialization.phase,
                    initialization.terminated_early,
                    complete,
                )),
            };
            let first = ConstructionConfig {
                max_steps: Some(1),
                assert_scores: true,
                ..Default::default()
            };
            let phases = if mode == 5 {
                vec![construction(first), $exact]
            } else {
                vec![
                    construction(first),
                    construction(ConstructionConfig {
                        assert_scores: true,
                        ..Default::default()
                    }),
                    local(LocalSearchConfig {
                        max_steps: 0,
                        ..Default::default()
                    }),
                ]
            };
            let plan = $Plan {
                phases,
                limits: SolverLimits {
                    max_evaluations: if mode == 3 { 2 } else { 1000 },
                    max_move_evaluations: if mode == 2 { Some(1) } else { None },
                    cancellation: Some(&token),
                    termination: if mode == 8 { Some(&condition) } else { None },
                    ..Default::default()
                },
            };
            let mut extensions = SolverExtensions::default();
            extensions.move_providers.bind(0, &mut provider);
            if mode == 1 {
                extensions.move_providers.bind(1, &mut second);
            }
            if mode == 4 {
                extensions.move_providers.max_construction_scans = 1;
            }
            if mode == 7 {
                extensions.move_providers.max_construction_scans = 0;
            }
            extensions.events = Some(&mut events);
            let r = $solve(&p, $objective, plan, extensions).unwrap();
            assert_eq!(
                r.work.completed_steps, 0,
                "construction must not consume search steps"
            );
            match mode {
                2 | 3 | 6 => {
                    assert_eq!(r.phases.len(), 1);
                    assert!(!r.complete);
                    assert_eq!($count(&r.assignment), 1);
                    assert_eq!(
                        r.termination,
                        match mode {
                            2 => SolverTermination::MoveEvaluationLimit,
                            3 => SolverTermination::EvaluationLimit,
                            _ => SolverTermination::Cancelled,
                        }
                    );
                    assert_eq!(r.work.evaluations, 2);
                    assert_eq!(r.work.move_evaluations, 1);
                    assert!(initialized.is_empty());
                }
                _ => {
                    assert!(r.complete);
                    assert_eq!($count(&r.assignment), 2);
                    assert_eq!(r.score, Some(SimpleScore::new(0)));
                    assert_eq!(
                        r.termination,
                        if mode == 5 {
                            SolverTermination::Optimal
                        } else {
                            SolverTermination::PlanCompleted
                        }
                    );
                    if mode != 5 {
                        assert_eq!(initialized, [(1, false, true)]);
                    }
                }
            }
            assert_eq!(
                r.phases[0].termination,
                PhaseTermination::Construction(match mode {
                    2 => ConstructionTermination::MoveEvaluationLimit,
                    3 => ConstructionTermination::EvaluationLimit,
                    4 | 7 => ConstructionTermination::ScanLimit,
                    5 => ConstructionTermination::NoCandidates,
                    6 => ConstructionTermination::Cancelled,
                    _ => ConstructionTermination::StepLimit,
                })
            );
            if mode == 7 {
                assert!(seen.is_empty());
            } else {
                assert_eq!(seen, [(0, 0)]);
            }
            if mode == 1 {
                assert_eq!(second_seen, [(0, 1)]);
            }
            assert_eq!(best.first(), Some(&(0, 0, false)));
            if !matches!(mode, 5 | 7) {
                assert!(best.contains(&(0, 1, false)));
            }
        }
        // Incomplete handoff policy remains in force for a provider that cannot fill.
        let mut empty = |_: &$P, _: &$A, _| -> Vec<$M> { vec![] };
        let mut extensions = SolverExtensions::default();
        extensions.move_providers.bind(0, &mut empty);
        let plan = $Plan {
            phases: vec![
                construction(ConstructionConfig::default()),
                local(LocalSearchConfig::default()),
            ],
            limits: SolverLimits {
                incomplete_handoff: IncompleteHandoffPolicy::Error,
                ..Default::default()
            },
        };
        assert!(matches!(
            $solve(&p, $objective, plan, extensions),
            Err(SolverError::IncompleteLocalSearch { phase: 1 })
        ));
        // A complete provider run records successive step snapshots and only committed states.
        let mut seen = vec![];
        let mut provider = |_: &$P, a: &$A, step| {
            seen.push((step, $count(a)));
            vec![$movement(a)]
        };
        let mut extensions = SolverExtensions::default();
        extensions.move_providers.bind(0, &mut provider);
        let plan = $Plan {
            phases: vec![
                construction(ConstructionConfig::default()),
                local(LocalSearchConfig {
                    max_steps: 0,
                    ..Default::default()
                }),
            ],
            ..Default::default()
        };
        let r = $solve(&p, $objective, plan, extensions).unwrap();
        assert!(r.complete);
        assert_eq!(seen, [(0, 0), (1, 1)]);
        assert_eq!(r.phases[0].work.evaluations, 3);
        assert_eq!(r.phases[0].work.move_evaluations, 2);
        assert_eq!(
            r.phases[0].termination,
            PhaseTermination::Construction(ConstructionTermination::Completed)
        );
    }};
}
#[test]
fn basic_scoped_plans() {
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    exercise!(
        FiniteProblem<i32>,
        Assignment<i32>,
        Move<i32>,
        basic_problem(),
        &n,
        solve_with_extensions,
        SolverSequence,
        SolverPhase::Construction,
        SolverPhase::LocalSearch,
        SolverPhase::Exact(ExactPhaseConfig::default()),
        basic_move,
        basic_count
    );
}
#[test]
fn basic_stream_plans() {
    let mut graph = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = b.source::<usize, (usize, Option<i32>)>();
        Ok((b.build(), s))
    };
    exercise!(
        FiniteProblem<i32>,
        Assignment<i32>,
        Move<i32>,
        basic_problem(),
        &mut graph,
        solve_stream_with_extensions,
        SolverSequence,
        SolverPhase::Construction,
        SolverPhase::LocalSearch,
        SolverPhase::Exact(ExactPhaseConfig::default()),
        basic_move,
        basic_count
    );
}
#[test]
fn list_scoped_plans() {
    let n = ListConstraintNetwork::<_, SimpleScore>::new(1, 2);
    exercise!(
        ListProblem<i32>,
        ListAssignment<i32>,
        ListMove<i32>,
        list_problem(),
        &n,
        solve_list_with_extensions,
        SolverSequence,
        SolverPhase::Construction,
        SolverPhase::LocalSearch,
        SolverPhase::Exact(ExactPhaseConfig::default()),
        list_move,
        list_count
    );
}
#[test]
fn list_stream_plans() {
    let mut graph = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = ListStreamSources::new(&mut b);
        Ok((b.build(), s))
    };
    exercise!(
        ListProblem<i32>,
        ListAssignment<i32>,
        ListMove<i32>,
        list_problem(),
        &mut graph,
        solve_list_stream_with_extensions,
        SolverSequence,
        SolverPhase::Construction,
        SolverPhase::LocalSearch,
        SolverPhase::Exact(ExactPhaseConfig::default()),
        list_move,
        list_count
    );
}
fn mixed_construction(c: ConstructionConfig<'_>) -> MixedSolverPhase<'_, SimpleScore> {
    MixedSolverPhase::Construction(MixedConstructionConfig {
        construction: c,
        ..Default::default()
    })
}
fn mixed_local(c: LocalSearchConfig<'_>) -> MixedSolverPhase<'_, SimpleScore> {
    MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
        search: c,
        ..Default::default()
    })
}
#[test]
fn mixed_scoped_plans() {
    let n = MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 1, 1);
    exercise!(MixedProblem<i32,i32>,MixedAssignment<i32,i32>,MixedMove<i32,i32>,mixed_problem(),&n,solve_mixed_with_extensions,MixedSolverSequence,
        mixed_construction,mixed_local,MixedSolverPhase::Exact(ExactPhaseConfig::default()),mixed_move,mixed_count);
}
#[test]
fn mixed_stream_plans() {
    let mut graph = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = MixedStreamSources::new(&mut b);
        Ok((b.build(), s))
    };
    exercise!(MixedProblem<i32,i32>,MixedAssignment<i32,i32>,MixedMove<i32,i32>,mixed_problem(),&mut graph,solve_mixed_stream_with_extensions,MixedSolverSequence,
        mixed_construction,mixed_local,MixedSolverPhase::Exact(ExactPhaseConfig::default()),mixed_move,mixed_count);
}
#[test]
fn zero_budget_validates_binding_without_provider_or_event_callbacks() {
    let p = basic_problem();
    let factories = Cell::new(0);
    let mut graph = || {
        factories.set(factories.get() + 1);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = b.source::<usize, (usize, Option<i32>)>();
        Ok((b.build(), s))
    };
    for invalid in [false, true] {
        let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _| -> Vec<Move<i32>> {
            panic!("provider")
        };
        let mut events = |_: SolverEvent<'_, Assignment<i32>, SimpleScore>| panic!("events");
        let mut extensions = SolverExtensions::default();
        extensions
            .move_providers
            .bind(if invalid { 1 } else { 0 }, &mut provider);
        extensions.events = Some(&mut events);
        let plan = SolverSequence {
            phases: vec![
                SolverPhase::Construction(ConstructionConfig::default()),
                SolverPhase::Exact(ExactPhaseConfig::default()),
            ],
            limits: SolverLimits {
                max_evaluations: 0,
                ..Default::default()
            },
        };
        let r = solve_stream_with_extensions(&p, &mut graph, plan, extensions);
        if invalid {
            assert!(matches!(
                r,
                Err(SolverError::InvalidMoveProviderPhase { phase: 1 })
            ));
        } else {
            assert_eq!(r.unwrap().termination, SolverTermination::EvaluationLimit);
        }
    }
    // Incomplete initial assignments still validate a fresh graph at zero budget.
    // Invalid provider phase bindings are rejected before that factory.
    assert_eq!(factories.get(), 1);
}
