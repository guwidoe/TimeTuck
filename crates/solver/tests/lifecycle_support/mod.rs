// Original backend harness; config/count helpers adapt pinned PlannerTestUtils and
// PlannerAssert, constant zero scoring adapts DummySimpleScoreEasyScoreCalculator.
// Apache-2.0; see compat/provenance.json.
#![allow(dead_code)]
use std::marker::PhantomData;
use timetuck::*;
pub fn config() -> SolverConfig<'static> {
    SolverConfig {
        local_search: vec![LocalSearchConfig {
            max_steps: 10,
            acceptance: Acceptance::HillClimbing,
            ..Default::default()
        }],
        ..Default::default()
    }
}
pub fn counts(events: &[SolverLifecycleEvent], solves: usize, phases: usize, steps: usize) {
    use SolverLifecycleEvent::*;
    for count in [
        events.iter().filter(|e| matches!(e, SolveStarted)).count(),
        events.iter().filter(|e| matches!(e, SolveEnded)).count(),
    ] {
        assert_eq!(count, solves);
    }
    for count in [
        events
            .iter()
            .filter(|e| matches!(e, PhaseStarted { .. }))
            .count(),
        events
            .iter()
            .filter(|e| matches!(e, PhaseEnded { .. }))
            .count(),
    ] {
        assert_eq!(count, phases);
    }
    for count in [
        events
            .iter()
            .filter(|e| matches!(e, StepStarted { .. }))
            .count(),
        events
            .iter()
            .filter(|e| matches!(e, StepEnded { .. }))
            .count(),
    ] {
        assert_eq!(count, steps);
    }
}
pub struct Zero<C>(PhantomData<C>);
impl<C> Default for Zero<C> {
    fn default() -> Self {
        Self(PhantomData)
    }
}
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
pub fn basic_graph() -> (StreamGraph<'static, SimpleScore>, PlanningStreamSource<i32>) {
    let mut b = StreamGraphBuilder::new();
    let s = b.source();
    (b.build(), s)
}
pub fn list_graph() -> (StreamGraph<'static, SimpleScore>, ListStreamSources<i32>) {
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::new(&mut b);
    (b.build(), s)
}
pub fn mixed_graph() -> (
    StreamGraph<'static, SimpleScore>,
    MixedStreamSources<i32, i32>,
) {
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::new(&mut b);
    (b.build(), s)
}
pub fn basic_run<'a>(
    p: &FiniteProblem<i32>,
    backend: usize,
    c: SolverConfig<'_>,
    callback: Option<&'a mut SolverLifecycleCallback<'a>>,
) -> SolverResult<Assignment<i32>, SimpleScore> {
    match backend {
        0 => solve_with_extensions(
            p,
            &ConstraintNetwork::<_, SimpleScore>::new(p.variables().len()),
            c,
            SolverExtensions {
                lifecycle: callback,
                ..Default::default()
            },
        )
        .unwrap(),
        1 => solve_stream_with_extensions(
            p,
            || Ok(basic_graph()),
            c,
            SolverExtensions {
                lifecycle: callback,
                ..Default::default()
            },
        )
        .unwrap(),
        2 => solve_calculator_with_extensions(
            p,
            &Zero::<Vec<usize>>::default,
            c,
            SolverExtensions {
                lifecycle: callback,
                ..Default::default()
            },
        )
        .unwrap(),
        _ => {
            let (g, s) = basic_graph();
            StreamScoreDirector::new(p, g, s, p.initial_assignment())
                .unwrap()
                .run_solver_with_extensions(
                    c,
                    SolverExtensions {
                        lifecycle: callback,
                        ..Default::default()
                    },
                )
                .unwrap()
        }
    }
}
pub fn list_run<'a>(
    p: &ListProblem<i32>,
    backend: usize,
    callback: Option<&'a mut SolverLifecycleCallback<'a>>,
) -> SolverResult<ListAssignment<i32>, SimpleScore> {
    match backend {
        0 => solve_list_with_extensions(
            p,
            &ListConstraintNetwork::<_, SimpleScore>::new(p.entities().len(), p.values().len()),
            config(),
            SolverExtensions {
                lifecycle: callback,
                ..Default::default()
            },
        )
        .unwrap(),
        1 => solve_list_stream_with_extensions(
            p,
            || Ok(list_graph()),
            config(),
            SolverExtensions {
                lifecycle: callback,
                ..Default::default()
            },
        )
        .unwrap(),
        2 => solve_list_calculator_with_extensions(
            p,
            &Zero::<Vec<usize>>::default,
            config(),
            SolverExtensions {
                lifecycle: callback,
                ..Default::default()
            },
        )
        .unwrap(),
        _ => {
            let (g, s) = list_graph();
            ListStreamScoreDirector::new(p, g, s, p.initial_assignment())
                .unwrap()
                .run_solver_with_extensions(
                    config(),
                    SolverExtensions {
                        lifecycle: callback,
                        ..Default::default()
                    },
                )
                .unwrap()
        }
    }
}
pub fn mixed_run<'a>(
    p: &MixedProblem<i32, i32>,
    backend: usize,
    callback: Option<&'a mut SolverLifecycleCallback<'a>>,
) -> SolverResult<MixedAssignment<i32, i32>, SimpleScore> {
    let c = MixedSolverConfig {
        local_search: vec![MixedLocalSearchConfig {
            search: config().local_search.remove(0),
            ..Default::default()
        }],
        ..Default::default()
    };
    match backend {
        0 => solve_mixed_with_extensions(
            p,
            &MixedConstraintNetwork::<_, _, SimpleScore>::new(
                p.basic().variables().len(),
                p.lists().entities().len(),
                p.lists().values().len(),
            ),
            c,
            SolverExtensions {
                lifecycle: callback,
                ..Default::default()
            },
        )
        .unwrap(),
        1 => solve_mixed_stream_with_extensions(
            p,
            || Ok(mixed_graph()),
            c,
            SolverExtensions {
                lifecycle: callback,
                ..Default::default()
            },
        )
        .unwrap(),
        2 => solve_mixed_calculator_with_extensions(
            p,
            &Zero::<MixedCalculatorChange>::default,
            c,
            SolverExtensions {
                lifecycle: callback,
                ..Default::default()
            },
        )
        .unwrap(),
        _ => {
            let (g, s) = mixed_graph();
            MixedStreamScoreDirector::new(p, g, s, p.initial_assignment())
                .unwrap()
                .run_solver_with_extensions(
                    c,
                    SolverExtensions {
                        lifecycle: callback,
                        ..Default::default()
                    },
                )
                .unwrap()
        }
    }
}
