//! Original owned XML threshold controls; no upstream-method credit.
use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use std::time::Duration;
use timetuck::*;
type Output = SolverResult<Assignment<i32>, SimpleScore>;
fn plan(body: &str) -> XmlTerminationPlan {
    SolverXmlConfiguration::parse(&format!("<solver>{body}</solver>"))
        .unwrap()
        .native_termination_plan()
        .unwrap()
}
fn template() -> SolverSequence<'static> {
    SolverSequence {
        phases: [1, 10]
            .into_iter()
            .map(|max_steps| {
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps,
                    max_evaluations: 100,
                    acceptance: Acceptance::HillClimbing,
                    selection: MoveSelection::FirstAccepted,
                    assert_scores: true,
                    ..Default::default()
                })
            })
            .collect(),
        ..Default::default()
    }
}
fn factory(
    stream: bool,
    body: &str,
    calls: Arc<AtomicUsize>,
) -> XmlSolverFactory<FiniteProblem<i32>, Output, String> {
    XmlSolverFactory::new_with_controls(
        template(),
        plan(body),
        move |problem: FiniteProblem<i32>, sequence, bindings, stateful, progress| {
            calls.fetch_add(1, Ordering::SeqCst);
            if problem.variables().len() != 2 {
                return Err("expected two variables".into());
            }
            let now = Arc::new(AtomicU64::new(0));
            let read = now.clone();
            let clock =
                SolverClock::new(move || Duration::from_millis(read.load(Ordering::SeqCst)));
            let mut lifecycle = |event| match event {
                SolverLifecycleEvent::StepEnded {
                    completed: true, ..
                } => {
                    now.fetch_add(600, Ordering::SeqCst);
                }
                SolverLifecycleEvent::PhaseEnded { phase: 0, .. } => {
                    now.fetch_add(500, Ordering::SeqCst);
                }
                _ => {}
            };
            let result = if stream {
                solve_stream_with_extensions(
                    &problem,
                    || {
                        let mut b = StreamGraphBuilder::new();
                        let source = b.source::<usize, (usize, Option<i32>)>();
                        b.score("flat", source.port(), |_| SimpleScore::ZERO)?;
                        Ok((b.build(), source))
                    },
                    sequence,
                    SolverExtensions {
                        phase_terminations: bindings,
                        stateful_termination: stateful,
                        clock: Some(&clock),
                        lifecycle: Some(&mut lifecycle),
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())?
            } else {
                let n = ConstraintNetwork::<i32, SimpleScore>::new(2);
                solve_with_extensions(
                    &problem,
                    &n,
                    sequence,
                    SolverExtensions {
                        phase_terminations: bindings,
                        stateful_termination: stateful,
                        clock: Some(&clock),
                        lifecycle: Some(&mut lifecycle),
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())?
            };
            problem.validate(&result.assignment).unwrap();
            // Independent flat objective, including native early-cap partial outputs.
            assert!(result.score.is_none_or(|s| s == SimpleScore::ZERO));
            progress.record_work(result.work);
            Ok(result)
        },
    )
    .unwrap()
}
fn problem() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 2]).unwrap()
}
const THRESHOLD: &str = "<unimprovedMillisecondsSpentLimit>1000</unimprovedMillisecondsSpentLimit><unimprovedScoreDifferenceThreshold>10</unimprovedScoreDifferenceThreshold>";
#[test]
fn owned_root_history_crosses_handoff_and_each_job_starts_fresh() {
    for stream in [false, true] {
        let calls = Arc::new(AtomicUsize::new(0));
        let factory = factory(
            stream,
            &format!("<termination>{THRESHOLD}</termination>"),
            calls.clone(),
        );
        let mut manager = factory.build_manager(1).unwrap();
        drop(factory);
        for id in 0..3 {
            let job = manager
                .job_builder(id)
                .with_problem(problem())
                .run()
                .unwrap();
            let result = job.wait();
            let result = result.as_ref().as_ref().unwrap();
            assert_eq!(result.work.completed_steps, 1);
            assert_eq!(result.score, Some(SimpleScore::ZERO));
            assert_eq!(result.termination, SolverTermination::Condition);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        manager.shutdown().unwrap();
    }
}
#[test]
fn root_grouping_phase_histories_and_native_caps_remain_distinct() {
    for stream in [false, true] {
        for (body, expected) in [
            (
                format!(
                    "<termination><terminationCompositionStyle>AND</terminationCompositionStyle>{THRESHOLD}<stepCountLimit>3</stepCountLimit></termination>"
                ),
                3,
            ),
            (
                format!(
                    "<termination><terminationCompositionStyle>OR</terminationCompositionStyle>{THRESHOLD}<stepCountLimit>3</stepCountLimit></termination>"
                ),
                1,
            ),
            (
                format!(
                    "<localSearch><termination>{THRESHOLD}</termination></localSearch><localSearch><termination>{THRESHOLD}</termination></localSearch>"
                ),
                3,
            ),
        ] {
            let owned = factory(stream, &body, Arc::new(AtomicUsize::new(0)));
            let mut manager = owned.build_manager(1).unwrap();
            let job = manager
                .job_builder(1)
                .with_problem(problem())
                .run()
                .unwrap();
            let result = job.wait();
            assert_eq!(
                result.as_ref().as_ref().unwrap().work.completed_steps,
                expected
            );
            let capped = manager
                .job_builder(2)
                .with_problem(problem())
                .with_time_override(SolverTimeOverride {
                    max_time: Some(Duration::ZERO),
                    ..Default::default()
                })
                .run()
                .unwrap()
                .wait();
            let capped = capped.as_ref().as_ref().unwrap();
            assert_eq!(capped.work.completed_steps, 0);
            assert_eq!(capped.termination, SolverTermination::TimeLimit);
            manager.shutdown().unwrap();
        }
    }
}
#[test]
fn invalid_job_does_not_poison_next_threshold_job() {
    for stream in [false, true] {
        let owned = factory(
            stream,
            &format!("<termination>{THRESHOLD}</termination>"),
            Arc::new(AtomicUsize::new(0)),
        );
        let mut manager = owned.build_manager(1).unwrap();
        let bad = FiniteProblem::new(vec![Variable::new(vec![0]).with_initial(0)]).unwrap();
        let failed = manager
            .job_builder(1)
            .with_problem(bad)
            .run()
            .unwrap()
            .wait();
        assert!(matches!(
            failed.as_ref(),
            Err(SolverJobError::Solve(XmlSolverRunError::Solve(_)))
        ));
        let result = manager
            .job_builder(2)
            .with_problem(problem())
            .run()
            .unwrap()
            .wait();
        assert_eq!(result.as_ref().as_ref().unwrap().work.completed_steps, 1);
        manager.shutdown().unwrap();
    }
}
#[test]
fn recursive_threshold_preflight_and_legacy_scope_reject_before_executor() {
    let bad_score = plan(
        "<termination><termination><unimprovedMillisecondsSpentLimit>1000</unimprovedMillisecondsSpentLimit><unimprovedScoreDifferenceThreshold>bad</unimprovedScoreDifferenceThreshold></termination></termination>",
    );
    // Public plans remain editable after XML validation; factory preflight must
    // reject a missing window even when the declaration came from native editing.
    let mut missing_window = plan(&format!(
        "<termination><termination>{THRESHOLD}</termination></termination>"
    ));
    missing_window.solver.as_mut().unwrap().terminations[0].unimproved = Default::default();
    for invalid in [bad_score, missing_window] {
        let result: Result<XmlSolverFactory<(), (), String>, _> =
            XmlSolverFactory::new_with_controls(template(), invalid, |_, _, _, _, _| {
                panic!("preflight reached executor")
            });
        assert!(matches!(
            result,
            Err(XmlSolverFactoryBuildError::Binding(_))
        ));
    }
    let result: Result<XmlSolverFactory<(), (), String>, _> = XmlSolverFactory::new(
        template(),
        plan(&format!("<termination>{THRESHOLD}</termination>")),
        |_, _, _, _| panic!("legacy root reached executor"),
    );
    assert!(matches!(
        result,
        Err(XmlSolverFactoryBuildError::Binding(_))
    ));
}
