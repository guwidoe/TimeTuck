//! Original owned plan/manager execution and reference lifetime controls.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use timetuck::*;
type BasicResult = SolverResult<Assignment<i32>, SimpleScore>;
type ListResult = SolverResult<ListAssignment<i32>, SimpleScore>;
fn plan(body: &str) -> XmlTerminationPlan {
    SolverXmlConfiguration::parse(&format!("<solver>{body}</solver>"))
        .unwrap()
        .native_termination_plan()
        .unwrap()
}
fn template(count: usize) -> SolverSequence<'static> {
    SolverSequence {
        phases: (0..count)
            .map(|_| {
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 10,
                    max_evaluations: 150,
                    selection: MoveSelection::BestAccepted,
                    assert_scores: true,
                    ..Default::default()
                })
            })
            .collect(),
        ..Default::default()
    }
}
fn basic_factory(
    stream: bool,
    xml: &str,
    count: usize,
    calls: Arc<AtomicUsize>,
) -> XmlSolverFactory<FiniteProblem<i32>, BasicResult, String> {
    XmlSolverFactory::new(
        template(count),
        plan(xml),
        move |problem: FiniteProblem<i32>, config, bindings, progress| {
            calls.fetch_add(1, Ordering::SeqCst);
            let result = if stream {
                solve_stream_with_extensions(
                    &problem,
                    || {
                        let mut b = StreamGraphBuilder::new();
                        let source = b.source::<usize, (usize, Option<i32>)>();
                        b.score("sum", source.port(), |(_, v)| {
                            SimpleScore::new(i64::from(v.unwrap_or(0)))
                        })?;
                        Ok((b.build(), source))
                    },
                    config,
                    SolverExtensions {
                        phase_terminations: bindings,
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())?
            } else {
                let mut n = ConstraintNetwork::new(problem.variables().len());
                for i in 0..problem.variables().len() {
                    n.add_constraint(format!("v{i}"), vec![i], |v| {
                        SimpleScore::new(i64::from(*v[0].unwrap_or(&0)))
                    })
                    .unwrap();
                }
                solve_with_extensions(
                    &problem,
                    &n,
                    config,
                    SolverExtensions {
                        phase_terminations: bindings,
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())?
            };
            problem.validate(&result.assignment).unwrap();
            let score = SimpleScore::new(
                result
                    .assignment
                    .values()
                    .iter()
                    .map(|v| i64::from(v.unwrap()))
                    .sum(),
            );
            assert_eq!(result.score, Some(score));
            progress.record_work(result.work);
            Ok(result)
        },
    )
    .unwrap()
}
fn problem(max: i32) -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new((0..=max).collect()).with_initial(0); 3]).unwrap()
}
#[test]
fn owned_root_and_or_conditions_execute_independently_after_factory_wrapper_drops() {
    for stream in [false, true] {
        for (style, expected_steps) in [("AND", 2), ("OR", 1)] {
            let xml = format!(
                "<termination><terminationCompositionStyle>{style}</terminationCompositionStyle><stepCountLimit>2</stepCountLimit><bestScoreLimit>4</bestScoreLimit></termination>"
            );
            let calls = Arc::new(AtomicUsize::new(0));
            let owned = basic_factory(stream, &xml, 1, calls.clone());
            let clone = owned.clone();
            assert_eq!(owned.termination_plan(), clone.termination_plan());
            let mut manager = owned.build_manager(2).unwrap();
            drop(owned);
            drop(clone);
            let first = manager
                .job_builder(1)
                .with_problem(problem(4))
                .run()
                .unwrap();
            let second = manager
                .job_builder(2)
                .with_problem(problem(4))
                .run()
                .unwrap();
            for job in [first, second] {
                assert!(job.configuration().limits.scored_termination.is_none());
                let result = job.wait();
                let result = result.as_ref().as_ref().unwrap();
                assert_eq!(result.termination, SolverTermination::Condition);
                assert_eq!(result.work.completed_steps, expected_steps);
                assert_eq!(
                    result.score,
                    Some(SimpleScore::new(expected_steps as i64 * 4))
                );
            }
            assert_eq!(calls.load(Ordering::SeqCst), 2);
            manager.shutdown().unwrap();
        }
    }
}
#[test]
fn phase_conditions_are_installed_and_native_job_deadline_stays_independent() {
    for stream in [false, true] {
        let xml = "<localSearch><termination><terminationCompositionStyle>AND</terminationCompositionStyle><stepCountLimit>2</stepCountLimit><bestScoreLimit>4</bestScoreLimit></termination></localSearch><localSearch/>";
        let owned = basic_factory(stream, xml, 2, Arc::new(AtomicUsize::new(0)));
        let mut manager = owned.build_manager(1).unwrap();
        let job = manager
            .job_builder(1)
            .with_problem(problem(2))
            .run()
            .unwrap();
        let result = job.wait();
        let result = result.as_ref().as_ref().unwrap();
        assert_eq!(
            result.phases[0].termination,
            PhaseTermination::LocalSearch(LocalSearchTermination::StepCondition)
        );
        assert_eq!(result.phases[0].score, Some(SimpleScore::new(4)));
        assert_eq!(result.score, Some(SimpleScore::new(6)));
        let limited = manager
            .job_builder(2)
            .with_problem(problem(2))
            .with_time_override(SolverTimeOverride {
                max_time: Some(Duration::ZERO),
                ..Default::default()
            })
            .run()
            .unwrap();
        assert_eq!(
            limited.configuration().limits.max_time,
            Some(Duration::ZERO)
        );
        let limited = limited.wait();
        let limited = limited.as_ref().as_ref().unwrap();
        assert_eq!(limited.termination, SolverTermination::TimeLimit);
        assert_eq!(limited.work.completed_steps, 0);
        assert_eq!(owned.factory().configuration().limits.max_time, None);
        manager.shutdown().unwrap();
    }
}
#[test]
fn actual_list_directors_consume_owned_xml_step_expressions_in_both_backends() {
    for stream in [false, true] {
        let factory: XmlSolverFactory<ListProblem<i32>, ListResult, String> = XmlSolverFactory::new(
            template(1), plan("<termination><terminationCompositionStyle>AND</terminationCompositionStyle><stepCountLimit>2</stepCountLimit><bestScoreLimit>18</bestScoreLimit></termination>"),
            move |problem: ListProblem<i32>, config, bindings, progress| {
                let result = if stream {
                    solve_list_stream_with_extensions(&problem, || {
                        let mut b = StreamGraphBuilder::new();
                        let sources = ListStreamSources::<i32>::new(&mut b);
                        b.score("owners", sources.entities.port(), |(e, row)| SimpleScore::new((*e as i64 + 1) * row.iter().map(|&v| i64::from(v)).sum::<i64>()))?;
                        Ok((b.build(), sources))
                    }, config, SolverExtensions { phase_terminations: bindings, ..Default::default() }).map_err(|e| e.to_string())?
                } else {
                    let mut n = ListConstraintNetwork::new(3, 3);
                    for e in 0..3 {
                        n.add_constraint(format!("owner{e}"), vec![ListScope::Entity(e)], move |f| {
                            let ListFact::Entity(row) = f[0] else { unreachable!() };
                            SimpleScore::new((e as i64 + 1) * row.iter().map(|&v| i64::from(v)).sum::<i64>())
                        }).unwrap();
                    }
                    solve_list_with_extensions(&problem, &n, config, SolverExtensions { phase_terminations: bindings, ..Default::default() }).map_err(|e| e.to_string())?
                };
                problem.validate(&result.assignment).unwrap();
                let score: i64 = result.assignment.lists().iter().enumerate().map(|(e, row)| (e as i64+1)*row.iter().map(|&v| i64::from(v)).sum::<i64>()).sum();
                assert_eq!(result.score, Some(SimpleScore::new(score)));
                progress.record_work(result.work);
                Ok(result)
            }).unwrap();
        let problem = ListProblem::new(
            vec![1_i32, 2, 3],
            vec![
                ListEntity::new(vec![1]),
                ListEntity::new(vec![2]),
                ListEntity::new(vec![3]),
            ],
        )
        .unwrap();
        let mut manager = factory.build_manager(1).unwrap();
        let job = manager.job_builder(1).with_problem(problem).run().unwrap();
        let result = job.wait();
        let result = result.as_ref().as_ref().unwrap();
        assert_eq!(result.termination, SolverTermination::Condition);
        assert_eq!(result.score, Some(SimpleScore::new(18)));
        assert_eq!(result.work.completed_steps, 2);
        manager.shutdown().unwrap();
    }
}
#[test]
fn invalid_xml_bindings_reject_factory_publication_without_executor_calls() {
    for xml in [
        "<termination><bestScoreLimit>bad</bestScoreLimit></termination>",
        "<constructionHeuristic/>",
        "<termination><terminationCompositionStyle>AND</terminationCompositionStyle><diminishedReturns/><termination><bestScoreLimit>bad</bestScoreLimit></termination></termination>",
    ] {
        let result: Result<XmlSolverFactory<FiniteProblem<i32>, (), String>, _> =
            XmlSolverFactory::new(template(1), plan(xml), |_, _, _, _| {
                panic!("invalid declaration reached executor")
            });
        assert!(matches!(
            result,
            Err(XmlSolverFactoryBuildError::Binding(_))
        ));
    }
}
