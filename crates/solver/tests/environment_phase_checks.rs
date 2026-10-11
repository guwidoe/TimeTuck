//! Original binding regressions over actual native solver backends.
use std::{cell::Cell, marker::PhantomData};
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
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::ZERO
    }
}
fn plan() -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![
            SolverPhase::Construction(Default::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 0,
                ..Default::default()
            }),
        ],
        ..Default::default()
    }
}
fn mixed_plan() -> MixedSolverSequence<'static> {
    MixedSolverSequence {
        phases: vec![
            MixedSolverPhase::Construction(Default::default()),
            MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_steps: 0,
                    ..Default::default()
                },
                ..Default::default()
            }),
        ],
        ..Default::default()
    }
}
#[test]
fn selective_checks_skip_first_phase_and_detect_second_phase_corruption_in_all_nine_backends() {
    let b = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let l = ListProblem::new(vec![7], vec![ListEntity::new(vec![7])]).unwrap();
    let m = MixedProblem::new(b.clone(), l.clone());
    let bn = ConstraintNetwork::<_, SimpleScore>::new(1);
    let ln = ListConstraintNetwork::<_, SimpleScore>::new(1, 1);
    let mn = MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 1, 1);
    macro_rules! run {
        ($solve:ident,$p:expr,$resource:expr,$plan:expr,$A:ty) => {
            for enabled in [false, true] {
                let p = EnvironmentProfile::new(
                    Some(EnvironmentMode::NoAssert),
                    vec![None, enabled.then_some(EnvironmentMode::PhaseAssert)],
                )
                .prepare()
                .unwrap();
                let calls = Cell::new(0);
                // Deliberately wrong independent expectation must be ignored by
                // NoAssert, then detected only at the enabled second handoff.
                let mut check = p
                    .score_check(|_: &$A| {
                        calls.set(calls.get() + 1);
                        Ok(SimpleScore::new(1))
                    })
                    .unwrap();
                let result = $solve(
                    &$p,
                    $resource,
                    $plan,
                    SolverExtensions {
                        score_check: Some(&mut check),
                        ..Default::default()
                    },
                );
                assert_eq!(calls.get(), usize::from(enabled));
                if enabled {
                    assert!(matches!(
                        result,
                        Err(SolverError::PhaseScoreMismatch {
                            phase: 1,
                            kind: SolverPhaseKind::LocalSearch,
                            ..
                        })
                    ));
                } else {
                    let r = result.unwrap();
                    assert_eq!(r.score, Some(SimpleScore::ZERO));
                    assert!(r.complete);
                    assert_eq!(r.phases.len(), 2);
                }
            }
        };
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
        &|| Zero::<Vec<usize>>(PhantomData),
        plan(),
        Assignment<i32>
    );
    run!(
        solve_list_calculator_with_extensions,
        l,
        &|| Zero::<Vec<usize>>(PhantomData),
        plan(),
        ListAssignment<i32>
    );
    run!(solve_mixed_calculator_with_extensions,m,&||Zero::<MixedCalculatorChange>(PhantomData),mixed_plan(),MixedAssignment<i32,i32>);
    run!(
        solve_stream_with_extensions,
        b,
        || {
            let mut g = StreamGraphBuilder::new();
            let s = g.source::<usize, (usize, Option<i32>)>();
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
            Ok((g.build(), s))
        },
        plan(),
        ListAssignment<i32>
    );
    run!(solve_mixed_stream_with_extensions,m,||{let mut g=StreamGraphBuilder::new();let s=MixedStreamSources::new(&mut g);Ok((g.build(),s))},mixed_plan(),MixedAssignment<i32,i32>);
}
#[test]
fn binding_rejects_unsupported_modes_and_preserves_initial_scope_and_scorer_errors() {
    use EnvironmentMode as M;
    for mode in [
        M::StepAssert,
        M::NonIntrusiveFullAssert,
        M::FullAssert,
        M::TrackedFullAssert,
    ] {
        let p = EnvironmentProfile::new(Some(M::NoAssert), vec![Some(mode)])
            .prepare()
            .unwrap();
        let Err(error) = p.score_check(|_: &()| Ok(SimpleScore::ZERO)) else {
            panic!("unsupported mode was bound")
        };
        assert_eq!(
            error,
            EnvironmentBindingError {
                phase: Some(0),
                mode
            }
        );
    }
    let p = EnvironmentProfile::new(Some(M::NonReproducible), vec![])
        .prepare()
        .unwrap();
    let Err(error) = p.score_check(|_: &()| Ok(SimpleScore::ZERO)) else {
        panic!("non-reproducible runtime was silently enabled")
    };
    assert_eq!(error.phase, None);
    let p = EnvironmentProfile::new(None, vec![None]).prepare().unwrap();
    let mut check = p
        .score_check(|_: &()| Err::<SimpleScore, _>("fresh scoring failed".into()))
        .unwrap();
    assert_eq!(
        check(usize::MAX, SolverPhaseKind::Initial, &()),
        Err("fresh scoring failed".into())
    );
    assert_eq!(
        check(1, SolverPhaseKind::LocalSearch, &()),
        Err("verification profile has no phase 1".into())
    );
}
