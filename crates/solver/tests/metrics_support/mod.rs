//! Original metric backend matrix; shares the existing lifecycle fixtures.
#![allow(dead_code)]
#[path = "../lifecycle_support/mod.rs"]
mod lifecycle;
use lifecycle::*;
use timetuck::*;
pub fn basic_run<'a>(
    p: &FiniteProblem<i32>,
    backend: usize,
    c: SolverConfig<'_>,
    callback: Option<&'a mut SolverLifecycleCallback<'a>>,
    metrics: Option<&'a SolverMetrics<SimpleScore>>,
) -> SolverResult<Assignment<i32>, SimpleScore> {
    match backend {
        0 => solve_with_extensions(
            p,
            &ConstraintNetwork::<_, SimpleScore>::new(p.variables().len()),
            c,
            SolverExtensions {
                lifecycle: callback,
                metrics,
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
                metrics,
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
                metrics,
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
                        metrics,
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
    metrics: Option<&'a SolverMetrics<SimpleScore>>,
) -> SolverResult<ListAssignment<i32>, SimpleScore> {
    match backend {
        0 => solve_list_with_extensions(
            p,
            &ListConstraintNetwork::<_, SimpleScore>::new(p.entities().len(), p.values().len()),
            config(),
            SolverExtensions {
                lifecycle: callback,
                metrics,
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
                metrics,
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
                metrics,
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
                        metrics,
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
    metrics: Option<&'a SolverMetrics<SimpleScore>>,
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
                metrics,
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
                metrics,
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
                metrics,
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
                        metrics,
                        ..Default::default()
                    },
                )
                .unwrap()
        }
    }
}
