//! Original regressions; no upstream method credit.
use std::marker::PhantomData;
use timetuck::*;

struct Zero<C>(PhantomData<C>);
impl<A, C> IncrementalCalculator<A> for Zero<C> {
    type Score = SimpleScore;
    type Justification = ();
    type Change = C;
    fn reset(
        &mut self,
        _: &A,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn before_change(
        &mut self,
        _: &A,
        _: &C,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn after_change(
        &mut self,
        _: &A,
        _: &C,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::ZERO
    }
}
#[derive(Debug, PartialEq)]
struct Outcome {
    work: SolverWork,
    stop: SolverTermination,
    complete: bool,
    assigned: usize,
    phases: usize,
}
fn run_all(mut config: SolverSequence<'_>, order: MixedConstructionOrder) -> Vec<Outcome> {
    // Phase diagnostics must preserve partial outputs, exact budgets and step counters.
    config.limits.assert_phase_scores = true;
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 4]).unwrap();
    let l = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
    )
    .unwrap();
    let m = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap(),
        ListProblem::new(
            vec![0, 1],
            vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    let mut out = vec![];
    macro_rules! record {
        ($r:expr, $count:expr) => {{
            let r = $r.unwrap();
            let assigned = $count(&r.assignment);
            if let Some(score) = r.score {
                assert_eq!(
                    score,
                    SimpleScore::ZERO.with_structural(assigned as i64 - 4)
                );
            }
            assert_eq!(
                r.work.construction_steps,
                r.phases.iter().map(|p| p.work.construction_steps).sum()
            );
            out.push(Outcome {
                work: r.work,
                stop: r.termination,
                complete: r.complete,
                assigned,
                phases: r.phases.len(),
            });
        }};
    }
    let basic_count = |a: &Assignment<i32>| a.values().iter().flatten().count();
    let list_count = |a: &ListAssignment<i32>| a.lists().iter().map(Vec::len).sum::<usize>();
    let mixed_count =
        |a: &MixedAssignment<i32, i32>| basic_count(a.basic()) + list_count(a.lists());
    record!(
        solve(
            &p,
            &ConstraintNetwork::<_, SimpleScore>::new(4),
            config.clone()
        ),
        basic_count
    );
    record!(
        solve_calculator(&p, &|| Zero::<Vec<usize>>(PhantomData), config.clone()),
        basic_count
    );
    record!(
        solve_stream(
            &p,
            || {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let s = b.source();
                Ok((b.build(), s))
            },
            config.clone()
        ),
        basic_count
    );
    record!(
        solve_list(
            &l,
            &ListConstraintNetwork::<_, SimpleScore>::new(2, 4),
            config.clone()
        ),
        list_count
    );
    record!(
        solve_list_calculator(&l, &|| Zero::<Vec<usize>>(PhantomData), config.clone()),
        list_count
    );
    record!(
        solve_list_stream(
            &l,
            || {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let s = ListStreamSources::new(&mut b);
                Ok((b.build(), s))
            },
            config.clone()
        ),
        list_count
    );
    let phases = config
        .phases
        .into_iter()
        .map(|p| match p {
            SolverPhase::Construction(config) => {
                MixedSolverPhase::Construction(MixedConstructionConfig {
                    construction: config,
                    order,
                })
            }
            SolverPhase::LocalSearch(search) => {
                MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                    search,
                    joint_moves: false,
                })
            }
            _ => panic!("fixture phase"),
        })
        .collect();
    let config = MixedSolverSequence {
        limits: config.limits,
        phases,
    };
    record!(
        solve_mixed(
            &m,
            &MixedConstraintNetwork::<_, _, SimpleScore>::new(2, 2, 2),
            config.clone()
        ),
        mixed_count
    );
    record!(
        solve_mixed_calculator(
            &m,
            &|| Zero::<MixedCalculatorChange>(PhantomData),
            config.clone()
        ),
        mixed_count
    );
    record!(
        solve_mixed_stream(
            &m,
            || {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let s = MixedStreamSources::new(&mut b);
                Ok((b.build(), s))
            },
            config
        ),
        mixed_count
    );
    out
}
fn config<'a>(
    condition: &'a SearchTermination<'a>,
    construction: ConstructionConfig<'a>,
) -> SolverSequence<'a> {
    SolverSequence {
        limits: SolverLimits {
            termination: Some(condition),
            ..Default::default()
        },
        phases: vec![SolverPhase::Construction(construction); 6],
    }
}
#[test]
fn construction_budget_spans_phases_placements_and_all_nine_backends() {
    for limit in 0..=5 {
        let condition = SearchTermination::Condition(SearchLimit::ConstructionSteps(limit));
        for placement in [
            ConstructionPlacement::Sequential,
            ConstructionPlacement::Pooled,
            ConstructionPlacement::ValueQueue,
        ] {
            for order in [
                MixedConstructionOrder::BasicThenLists,
                MixedConstructionOrder::ListsThenBasic,
            ] {
                let plan = config(
                    &condition,
                    ConstructionConfig {
                        max_steps: Some(1),
                        placement,
                        assert_scores: true,
                        ..Default::default()
                    },
                );
                for r in run_all(plan, order) {
                    assert_eq!(r.work.construction_steps, limit.min(4), "{r:?}");
                    assert_eq!(r.work.completed_steps, 0);
                    assert_eq!(r.assigned, limit.min(4) as usize);
                    assert_eq!(r.complete, limit >= 4);
                    assert_eq!(
                        r.stop,
                        if limit <= 4 {
                            SolverTermination::Condition
                        } else {
                            SolverTermination::PlanCompleted
                        }
                    );
                    assert_eq!(r.phases, if limit <= 4 { limit as usize } else { 6 });
                }
            }
        }
    }
}
#[test]
fn combined_budget_and_nested_conditions_cross_construction_search_handoff() {
    let children = [
        SearchTermination::Condition(SearchLimit::ConstructionSteps(4)),
        SearchTermination::Condition(SearchLimit::Steps(3)),
    ];
    for condition in [
        SearchTermination::Condition(SearchLimit::CombinedSteps(7)),
        SearchTermination::Condition(SearchLimit::Steps(3)),
        SearchTermination::All(&children),
    ] {
        let mut plan = config(
            &condition,
            ConstructionConfig {
                max_steps: Some(1),
                ..Default::default()
            },
        );
        plan.phases.truncate(4);
        plan.phases.extend(vec![
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 2,
                acceptance: Acceptance::HillClimbing,
                selection: MoveSelection::FirstAccepted,
                ..Default::default()
            });
            3
        ]);
        for r in run_all(plan, MixedConstructionOrder::BasicThenLists) {
            assert_eq!(r.work.construction_steps, 4, "{r:?}");
            assert_eq!(r.work.completed_steps, 3, "{r:?}");
            assert_eq!(r.stop, SolverTermination::Condition);
            assert_eq!(r.phases, 6);
            assert!(r.complete);
        }
    }
}
#[test]
fn interrupted_prefix_counts_but_discarded_scan_does_not() {
    let condition = SearchTermination::Condition(SearchLimit::ConstructionSteps(1));
    for interruption in [
        ConstructionInterruption::KeepBestPrefix,
        ConstructionInterruption::DiscardIncompleteStep,
    ] {
        let mut plan = config(
            &condition,
            ConstructionConfig {
                interruption,
                ..Default::default()
            },
        );
        plan.limits.max_move_evaluations = Some(1);
        for r in run_all(plan, MixedConstructionOrder::BasicThenLists) {
            let kept = interruption == ConstructionInterruption::KeepBestPrefix;
            assert_eq!(r.work.construction_steps, u64::from(kept), "{r:?}");
            assert_eq!(r.assigned, usize::from(kept));
            assert_eq!(r.work.move_evaluations, 1);
            assert_eq!(r.work.completed_steps, 0);
        }
    }
}
#[test]
fn progress_uses_exact_thresholds_even_when_combined_counter_overflows() {
    let condition = SearchLimit::CombinedSteps(u64::MAX);
    let before = condition
        .evaluate(SearchProgress {
            completed_steps: u64::MAX - 2,
            construction_steps: 1,
            ..Default::default()
        })
        .unwrap();
    assert!(!before.reached);
    let after = condition
        .evaluate(SearchProgress {
            completed_steps: u64::MAX - 2,
            construction_steps: 5,
            ..Default::default()
        })
        .unwrap();
    assert!(after.reached);
    assert_eq!(after.progress, Some(1.0));
}

#[test]
fn provider_composites_count_one_placement_in_all_nine_backends() {
    let condition = SearchTermination::Condition(SearchLimit::ConstructionSteps(1));
    let p = FiniteProblem::new(vec![Variable::new(vec![1]); 4]).unwrap();
    let l = ListProblem::new(vec![0, 1, 2, 3], vec![ListEntity::new(vec![])]).unwrap();
    let m = MixedProblem::new(p.clone(), l.clone());
    let movement = Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(1),
        },
        Move::Change {
            variable: 1,
            value: Some(1),
        },
    ]);
    let list_movement = ListMove::Composite(vec![
        ListMove::Assign {
            value: 0,
            destination: ListPosition {
                entity: 0,
                index: 0,
            },
        },
        ListMove::Assign {
            value: 1,
            destination: ListPosition {
                entity: 0,
                index: 1,
            },
        },
    ]);
    macro_rules! run {
        ($solve:ident, $p:expr, $resource:expr, $plan:expr, $P:ty, $A:ty, $movement:expr, $count:expr) => {{
            let mut calls = 0;
            let mut provider = |_: &$P, _: &$A, step: u64| {
                assert_eq!(step, 0);
                calls += 1;
                vec![$movement.clone()]
            };
            let mut ext = SolverExtensions::default();
            ext.move_providers.bind(0, &mut provider);
            let r = $solve($p, $resource, $plan, ext).unwrap();
            assert_eq!(calls, 1);
            assert_eq!(r.work.construction_steps, 1);
            assert_eq!(r.work.completed_steps, 0);
            assert_eq!(r.work.move_evaluations, 1);
            assert_eq!(r.phases.len(), 1);
            assert_eq!(r.termination, SolverTermination::Condition);
            assert_eq!($count(&r.assignment), 2);
            assert!(!r.complete);
        }};
    }
    let plan = config(
        &condition,
        ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        },
    );
    let count = |a: &Assignment<i32>| a.values().iter().flatten().count();
    run!(
        solve_with_extensions,
        &p,
        &ConstraintNetwork::<_, SimpleScore>::new(4),
        plan.clone(),
        FiniteProblem<i32>,
        Assignment<i32>,
        movement,
        count
    );
    run!(
        solve_calculator_with_extensions,
        &p,
        &|| Zero::<Vec<usize>>(PhantomData),
        plan.clone(),
        FiniteProblem<i32>,
        Assignment<i32>,
        movement,
        count
    );
    run!(
        solve_stream_with_extensions,
        &p,
        || {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let s = b.source();
            Ok((b.build(), s))
        },
        plan.clone(),
        FiniteProblem<i32>,
        Assignment<i32>,
        movement,
        count
    );
    let count = |a: &ListAssignment<i32>| a.lists().iter().map(Vec::len).sum::<usize>();
    run!(
        solve_list_with_extensions,
        &l,
        &ListConstraintNetwork::<_, SimpleScore>::new(1, 4),
        plan.clone(),
        ListProblem<i32>,
        ListAssignment<i32>,
        list_movement,
        count
    );
    run!(
        solve_list_calculator_with_extensions,
        &l,
        &|| Zero::<Vec<usize>>(PhantomData),
        plan.clone(),
        ListProblem<i32>,
        ListAssignment<i32>,
        list_movement,
        count
    );
    run!(
        solve_list_stream_with_extensions,
        &l,
        || {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let s = ListStreamSources::new(&mut b);
            Ok((b.build(), s))
        },
        plan.clone(),
        ListProblem<i32>,
        ListAssignment<i32>,
        list_movement,
        count
    );
    let plan = MixedSolverSequence {
        limits: plan.limits,
        phases: vec![MixedSolverPhase::Construction(MixedConstructionConfig::default()); 2],
    };
    let movement = MixedMove {
        basic: movement,
        lists: ListMove::Composite(vec![]),
    };
    let count = |a: &MixedAssignment<i32, i32>| a.basic().values().iter().flatten().count();
    run!(solve_mixed_with_extensions,&m,&MixedConstraintNetwork::<_,_,SimpleScore>::new(4,1,4),plan.clone(),MixedProblem<i32,i32>,MixedAssignment<i32,i32>,movement,count);
    run!(solve_mixed_calculator_with_extensions,&m,&|| Zero::<MixedCalculatorChange>(PhantomData),plan.clone(),MixedProblem<i32,i32>,MixedAssignment<i32,i32>,movement,count);
    run!(solve_mixed_stream_with_extensions,&m,|| { let mut b=StreamGraphBuilder::<SimpleScore>::new(); let s=MixedStreamSources::new(&mut b); Ok((b.build(),s)) },plan,MixedProblem<i32,i32>,MixedAssignment<i32,i32>,movement,count);
}

#[test]
fn construction_statistics_accumulate_reject_regression_and_report_overflow() {
    let mut manager = SolverManager::<(), String>::new(1).unwrap();
    let job = manager
        .submit_with_progress(
            1,
            |_, p| {
                assert!(p.record_work(SolverWork {
                    construction_steps: 2,
                    ..Default::default()
                }));
                assert!(!p.record_work(SolverWork {
                    construction_steps: 1,
                    ..Default::default()
                }));
                assert!(p.add_work(SolverWork {
                    construction_steps: 3,
                    ..Default::default()
                }));
                assert!(p.record_work(SolverWork {
                    construction_steps: 5,
                    ..Default::default()
                }));
                assert!(p.add_work(SolverWork {
                    construction_steps: u64::MAX,
                    ..Default::default()
                }));
                Ok(())
            },
            SolverJobCallbacks::default(),
        )
        .unwrap();
    assert!(job.wait().is_ok());
    let stats = job.statistics();
    assert!(stats.work_overflowed);
    assert_eq!(stats.work.unwrap().construction_steps, u64::MAX);
    manager.shutdown().unwrap();
}

#[test]
fn optional_no_change_is_a_completed_placement_without_an_assignment() {
    let condition = SearchTermination::Condition(SearchLimit::ConstructionSteps(1));
    let c = config(
        &condition,
        ConstructionConfig {
            optional: OptionalConstruction::EvaluateUnassigned,
            ..Default::default()
        },
    );
    let p = FiniteProblem::new(vec![Variable::new(vec![1]).allowing_unassigned(); 2]).unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("avoid", vec![0, 1], |v| {
        SimpleScore::new(-(v.iter().filter(|v| v.is_some()).count() as i64))
    })
    .unwrap();
    let r = solve(&p, &n, c.clone()).unwrap();
    assert_eq!(r.work.construction_steps, 1);
    assert_eq!(r.assignment.values(), &[None, None]);
    assert_eq!(r.termination, SolverTermination::Condition);
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![])])
        .unwrap()
        .allowing_unassigned();
    let mut n = ListConstraintNetwork::new(1, 2);
    n.add_constraint("avoid", vec![ListScope::Entity(0)], |v| {
        let ListFact::Entity(values) = &v[0] else {
            panic!()
        };
        SimpleScore::new(-(values.len() as i64))
    })
    .unwrap();
    let r = solve_list(&p, &n, c).unwrap();
    assert_eq!(r.work.construction_steps, 1);
    assert!(r.assignment.lists()[0].is_empty());
    assert_eq!(r.termination, SolverTermination::Condition);
}
