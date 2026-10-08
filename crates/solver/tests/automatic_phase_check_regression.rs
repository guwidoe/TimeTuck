//! Original automatic phase-score diagnostic regressions.
use std::{cell::Cell, marker::PhantomData};
use timetuck::*;
struct Tick<'a, C> {
    count: &'a Cell<i64>,
    score: i64,
    marker: PhantomData<C>,
}
impl<A, C> IncrementalCalculator<A> for Tick<'_, C> {
    type Score = SimpleScore;
    type Justification = ();
    type Change = C;
    fn reset(
        &mut self,
        _: &A,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.count.set(self.count.get() + 1);
        self.score = self.count.get();
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(self.score)
    }
}
fn tick<C>(count: &Cell<i64>) -> Tick<'_, C> {
    Tick {
        count,
        score: 0,
        marker: PhantomData,
    }
}
fn next(count: &Cell<i64>) -> SimpleScore {
    count.set(count.get() + 1);
    SimpleScore::new(count.get())
}
fn plan() -> SolverSequence<'static> {
    SolverSequence {
        limits: SolverLimits {
            assert_phase_scores: true,
            ..Default::default()
        },
        phases: vec![
            SolverPhase::Construction(Default::default()),
            SolverPhase::LocalSearch(Default::default()),
        ],
    }
}
fn mixed_plan() -> MixedSolverSequence<'static> {
    MixedSolverSequence {
        limits: SolverLimits {
            assert_phase_scores: true,
            ..Default::default()
        },
        phases: vec![
            MixedSolverPhase::Construction(Default::default()),
            MixedSolverPhase::LocalSearch(Default::default()),
        ],
    }
}
#[test]
fn nonrepeatable_scores_fail_before_handoff_initialization_or_external_check_in_all_nine_backends()
{
    let b = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let l = ListProblem::new(vec![7], vec![ListEntity::new(vec![7])]).unwrap();
    let m = MixedProblem::new(b.clone(), l.clone());
    let count = Cell::new(0);
    let mut bn = ConstraintNetwork::new(1);
    bn.add_constraint("tick", vec![0], |_| next(&count))
        .unwrap();
    let mut ln = ListConstraintNetwork::new(1, 1);
    ln.add_constraint("tick", vec![ListScope::Entity(0)], |_| next(&count))
        .unwrap();
    let mut mn = MixedConstraintNetwork::new(1, 1, 1);
    mn.add_constraint("tick", vec![MixedScope::Basic(0)], |_| next(&count))
        .unwrap();
    macro_rules! run {
        ($solve:ident,$p:expr,$resource:expr,$plan:expr,$A:ty) => {{
            count.set(0);
            let initialized = Cell::new(false);
            let checked = Cell::new(false);
            let mut event = |event: SolverEvent<'_, $A, SimpleScore>| {
                if matches!(event, SolverEvent::FirstInitialized { .. }) {
                    initialized.set(true)
                }
            };
            let mut check = |_, _, _: &$A| {
                checked.set(true);
                Ok(None)
            };
            let result = $solve(
                &$p,
                $resource,
                $plan,
                SolverExtensions {
                    events: Some(&mut event),
                    score_check: Some(&mut check),
                    ..Default::default()
                },
            );
            let Err(SolverError::PhaseScoreMismatch {
                phase,
                kind,
                actual,
                expected,
            }) = result
            else {
                panic!("expected score corruption")
            };
            assert_eq!(phase, 0);
            assert_eq!(kind, SolverPhaseKind::Construction);
            assert_ne!(actual, expected);
            assert_eq!(count.get(), 2);
            assert!(!initialized.get());
            assert!(!checked.get());
        }};
    }
    run!(solve_with_extensions, b, &bn, plan(), Assignment<i32>);
    run!(
        solve_list_with_extensions,
        l,
        &ln,
        plan(),
        ListAssignment<i32>
    );
    run!(solve_mixed_with_extensions,m,&mn,mixed_plan(),MixedAssignment<i32,i32>);
    run!(
        solve_calculator_with_extensions,
        b,
        &|| tick::<Vec<usize>>(&count),
        plan(),
        Assignment<i32>
    );
    run!(
        solve_list_calculator_with_extensions,
        l,
        &|| tick::<Vec<usize>>(&count),
        plan(),
        ListAssignment<i32>
    );
    run!(solve_mixed_calculator_with_extensions,m,&||tick::<MixedCalculatorChange>(&count),mixed_plan(),MixedAssignment<i32,i32>);
    run!(
        solve_stream_with_extensions,
        b,
        || {
            let mut g = StreamGraphBuilder::new();
            let s = g.source::<usize, (usize, Option<i32>)>();
            g.score("tick", s.port(), |_| next(&count))?;
            Ok((g.build(), s))
        },
        plan(),
        Assignment<i32>
    );
    run!(
        solve_list_stream_with_extensions,
        l,
        || {
            let mut g = StreamGraphBuilder::new();
            let s = ListStreamSources::new(&mut g);
            g.score("tick", s.values.port(), |_| next(&count))?;
            Ok((g.build(), s))
        },
        plan(),
        ListAssignment<i32>
    );
    run!(solve_mixed_stream_with_extensions,m,||{let mut g=StreamGraphBuilder::new();let s=MixedStreamSources::new(&mut g);g.score("tick",s.basic.port(),|_|next(&count))?;Ok((g.build(),s))},mixed_plan(),MixedAssignment<i32,i32>);
}
struct Drift<C>(i64, PhantomData<C>);
fn drift<C>() -> Drift<C> {
    Drift(0, PhantomData)
}
impl<A, C> IncrementalCalculator<A> for Drift<C> {
    type Score = SimpleScore;
    type Justification = ();
    type Change = C;
    fn reset(
        &mut self,
        _: &A,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.0 = 0;
        Ok(())
    }
    fn after_change(
        &mut self,
        _: &A,
        _: &C,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.0 += 10;
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(self.0)
    }
}
#[test]
fn fresh_calculators_detect_incremental_drift_in_basic_list_and_mixed_search() {
    let b = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let l = ListProblem::new(
        vec![7],
        vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
    )
    .unwrap();
    let m = MixedProblem::new(b.clone(), l.clone());
    let local = LocalSearchConfig {
        max_steps: 1,
        ..Default::default()
    };
    macro_rules! run {
        ($solve:ident,$p:expr,$factory:expr,$plan:expr) => {{
            let mut p = $plan;
            let plain = $solve(&$p, $factory, p.clone()).unwrap();
            assert!(plain.score.unwrap() > SimpleScore::ZERO);
            p.limits.assert_phase_scores = true;
            assert!(matches!(
                $solve(&$p, $factory, p),
                Err(SolverError::PhaseScoreMismatch {
                    phase: 0,
                    kind: SolverPhaseKind::LocalSearch,
                    ..
                })
            ));
        }};
    }
    run!(
        solve_calculator,
        b,
        &drift::<Vec<usize>>,
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(local)],
            ..Default::default()
        }
    );
    run!(
        solve_list_calculator,
        l,
        &drift::<Vec<usize>>,
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(local)],
            ..Default::default()
        }
    );
    run!(
        solve_mixed_calculator,
        m,
        &drift::<MixedCalculatorChange>,
        MixedSolverSequence {
            phases: vec![MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                search: local,
                ..Default::default()
            })],
            ..Default::default()
        }
    );
}
#[test]
fn initial_only_scoring_is_checked_but_zero_score_budget_never_creates_a_diagnostic() {
    let b = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
    ])
    .unwrap();
    let count = Cell::new(0);
    let mut p = plan();
    p.limits.max_evaluations = 1;
    assert!(matches!(
        solve_calculator(&b, &|| tick::<Vec<usize>>(&count), p.clone()),
        Err(SolverError::PhaseScoreMismatch {
            kind: SolverPhaseKind::Initial,
            ..
        })
    ));
    assert_eq!(count.get(), 2);
    count.set(0);
    p.limits.max_evaluations = 0;
    let r = solve_calculator(&b, &|| tick::<Vec<usize>>(&count), p).unwrap();
    assert_eq!(count.get(), 0);
    assert_eq!(r.work.evaluations, 0);
    assert!(r.phases.is_empty());
    let b = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let p = SolverSequence {
        limits: SolverLimits {
            assert_phase_scores: true,
            ..Default::default()
        },
        phases: vec![SolverPhase::Construction(ConstructionConfig {
            max_evaluations: 0,
            ..Default::default()
        })],
    };
    let r = solve_calculator(&b, &|| tick::<Vec<usize>>(&count), p).unwrap();
    assert_eq!(count.get(), 0);
    assert_eq!(r.score, None);
}

#[test]
fn diagnostic_factory_errors_and_panics_stop_before_later_phases() {
    for pinned in [false, true] {
        for panic in [false, true] {
            let b = FiniteProblem::new(vec![
                Variable::new(vec![0, 1])
                    .with_initial(0)
                    .with_pinned(pinned),
            ])
            .unwrap();
            let calls = Cell::new(0);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                solve_stream(
                    &b,
                    || {
                        calls.set(calls.get() + 1);
                        if calls.get() == 2 {
                            assert!(!panic, "diagnostic factory failure");
                            return Err(GraphError::Dependency(
                                "diagnostic factory failure".into(),
                            ));
                        }
                        let mut g = StreamGraphBuilder::<SimpleScore>::new();
                        let source = g.source();
                        Ok((g.build(), source))
                    },
                    plan(),
                )
            }));
            assert_eq!(calls.get(), 2);
            if panic {
                assert!(result.is_err());
            } else if pinned {
                assert!(matches!(
                    result.unwrap(),
                    Err(SolverError::InitialScoring { .. })
                ));
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(SolverError::Phase { phase: 0, .. })
                ));
            }
        }
    }
}
#[test]
fn diagnostic_scoring_does_not_reopen_providers_or_consume_exhausted_budgets() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2])]).unwrap();
    let calls = Cell::new(0);
    let scores = Cell::new(0);
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        scores.set(scores.get() + 1);
        SimpleScore::new(i64::from(v[0].copied().unwrap_or(0)))
    })
    .unwrap();
    let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
        calls.set(calls.get() + 1);
        vec![Move::Change {
            variable: 0,
            value: Some(2),
        }]
    };
    let mut ext = SolverExtensions::default();
    ext.move_providers.bind(0, &mut provider);
    let mut plan = plan();
    plan.limits.max_evaluations = 2;
    plan.limits.max_move_evaluations = Some(1);
    let r = solve_with_extensions(&p, &n, plan, ext).unwrap();
    assert!(r.complete);
    assert_eq!(r.score, Some(SimpleScore::new(2)));
    assert_eq!(r.work.evaluations, 2);
    assert_eq!(r.work.move_evaluations, 1);
    assert_eq!(r.phases.len(), 1);
    assert_eq!(calls.get(), 1);
    assert_eq!(scores.get(), 3);
}
