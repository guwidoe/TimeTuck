//! Original owned XML score-goal search contracts; no upstream-method credit.
use timetuck::*;
type Score = BendableScore<2, 1>;
type Output = SolverResult<Assignment<i32>, Score>;
const TARGET: &str = "<bestScoreLimit>[0/0]hard/[100]soft</bestScoreLimit>";
const FEASIBLE: &str = "<bestScoreFeasible>true</bestScoreFeasible>";
fn objective(value: i32) -> Score {
    match value {
        0 => Score::new([-10, -10], [0]),
        1 => Score::new([-1, -10], [0]),
        2 => Score::new([-1, -10], [-20]),
        _ => panic!("unexpected value"),
    }
}
fn factory(
    stream: bool,
    body: &str,
    phase_scope: bool,
) -> XmlSolverFactory<FiniteProblem<i32>, Output, String, Score> {
    let plan = SolverXmlConfiguration::parse(&format!("<solver>{body}</solver>"))
        .unwrap()
        .native_termination_plan()
        .unwrap();
    let template: SolverSequence<'static, Score> = SolverSequence {
        phases: (0..2)
            .map(|phase| {
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 1,
                    max_evaluations: 10,
                    acceptance: if phase_scope && phase == 0 {
                        Acceptance::HillClimbing
                    } else {
                        Acceptance::SimulatedAnnealingTermination {
                            starting_temperature: Score::new([20, 20], [20]),
                            scope: if phase_scope {
                                TerminationProgressScope::Phase
                            } else {
                                TerminationProgressScope::Solve
                            },
                            seed: 11,
                        }
                    },
                    assert_scores: true,
                    ..Default::default()
                })
            })
            .collect(),
        ..Default::default()
    };
    XmlSolverFactory::new_with_controls(
        template,
        plan,
        move |problem: FiniteProblem<i32>, sequence, bindings, stateful, progress| {
            let mut first = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
                vec![Move::Change {
                    variable: 0,
                    value: Some(1),
                }]
            };
            let mut second = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
                vec![Move::Change {
                    variable: 0,
                    value: Some(2),
                }]
            };
            let result = if stream {
                let mut extensions = SolverExtensions {
                    phase_terminations: bindings,
                    stateful_termination: stateful,
                    ..Default::default()
                };
                extensions.move_providers.bind(0, &mut first);
                extensions.move_providers.bind(1, &mut second);

                solve_stream_with_extensions(
                    &problem,
                    || {
                        let mut b = StreamGraphBuilder::new();
                        let source = b.source::<usize, (usize, Option<i32>)>();
                        b.score("objective", source.port(), |(_, value)| {
                            objective(value.unwrap())
                        })?;
                        Ok((b.build(), source))
                    },
                    sequence,
                    extensions,
                )
                .map_err(|error| error.to_string())
            } else {
                let mut extensions = SolverExtensions {
                    phase_terminations: bindings,
                    stateful_termination: stateful,
                    ..Default::default()
                };
                extensions.move_providers.bind(0, &mut first);
                extensions.move_providers.bind(1, &mut second);
                let mut network = ConstraintNetwork::<i32, Score>::new(1);
                network
                    .add_constraint("objective", vec![0], |v| objective(*v[0].unwrap()))
                    .unwrap();
                solve_with_extensions(&problem, &network, sequence, extensions)
                    .map_err(|error| error.to_string())
            }?;
            problem.validate(&result.assignment).unwrap();
            assert!(result.complete);
            assert_eq!(
                result.score,
                Some(objective(result.assignment.values()[0].unwrap()))
            );
            assert_eq!(result.assignment.values(), &[Some(1)]);
            assert_eq!(result.score, Some(Score::new([-1, -10], [0])));
            assert_eq!(result.termination, SolverTermination::PlanCompleted);
            progress.record_work(result.work);
            Ok(result)
        },
    )
    .unwrap()
}
fn run_jobs(stream: bool, body: &str, phase_scope: bool, expected: &[u64]) {
    let owned = factory(stream, body, phase_scope);
    let mut manager = owned.build_manager(1).unwrap();
    drop(owned);
    // A changed initialized baseline followed by the original job catches leaked
    // session state even though all three jobs have the same retained best score.
    for (id, (&initial, &steps)) in [0, 1, 0].iter().zip(expected).enumerate() {
        let problem =
            FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(initial)]).unwrap();
        let job = manager
            .job_builder(u64::try_from(id).unwrap())
            .with_problem(problem)
            .run()
            .unwrap();
        let output = job.wait();
        let result = output.as_ref().as_ref().unwrap();
        assert_eq!(result.phases.len(), 2);
        assert_eq!(
            result.phases[1].work.completed_steps, steps,
            "stream={stream}, job={id}, xml={body}"
        );
        assert_eq!(result.phases[1].work.move_evaluations, 1);
    }
    manager.shutdown().unwrap();
}
#[test]
fn owned_weighted_target_cools_across_handoff_and_rebases_each_job() {
    for stream in [false, true] {
        run_jobs(
            stream,
            &format!("<termination>{TARGET}</termination>"),
            false,
            &[0, 1, 0],
        );
        run_jobs(
            stream,
            "<termination><stepCountLimit>1000</stepCountLimit></termination>",
            false,
            &[1, 1, 1],
        );
    }
}
#[test]
fn owned_feasibility_cools_before_exact_feasibility_and_rebases_each_job() {
    for stream in [false, true] {
        run_jobs(
            stream,
            &format!("<termination>{FEASIBLE}</termination>"),
            false,
            &[0, 1, 0],
        );
    }
}
#[test]
fn owned_phase_goal_captures_a_fresh_baseline_after_handoff() {
    for stream in [false, true] {
        for goal in [TARGET, FEASIBLE] {
            run_jobs(
                stream,
                &format!(
                    "<localSearch/><localSearch><termination>{goal}</termination></localSearch>"
                ),
                true,
                &[1, 1, 1],
            );
        }
    }
}
#[test]
fn owned_nested_and_keeps_score_progress_when_step_stop_is_unreached() {
    for stream in [false, true] {
        run_jobs(
            stream,
            &format!(
                "<termination><terminationCompositionStyle>AND</terminationCompositionStyle><termination>{TARGET}</termination><stepCountLimit>10</stepCountLimit></termination>"
            ),
            false,
            &[1, 1, 1],
        );
    }
}
