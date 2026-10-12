//! Original callback-order, totals, failure and backend checks.
mod metrics_support;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn score(v: i64) -> SimpleScore {
    SimpleScore::new(v)
}
fn basic() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap()
}
fn network(calls: &Cell<usize>) -> ConstraintNetwork<'_, i32, SimpleScore> {
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        calls.set(calls.get() + 1);
        score(i64::from(v[0].copied().unwrap_or(0)))
    })
    .unwrap();
    n
}
fn graph(calls: &Cell<usize>) -> (StreamGraph<'_, SimpleScore>, PlanningStreamSource<i32>) {
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    b.score("value", source.port(), |(_, v)| {
        calls.set(calls.get() + 1);
        score(i64::from(v.unwrap_or(0)))
    })
    .unwrap();
    (b.build(), source)
}
fn sequence() -> SolverSequence<'static, SimpleScore> {
    SolverSequence {
        phases: vec![SolverPhase::Exact(ExactPhaseConfig {
            strategy: ExactSearchStrategy::Enumerate,
            ..Default::default()
        })],
        ..Default::default()
    }
}
fn check<A>(
    event: SolverEvent<'_, A, SimpleScore>,
    m: &SolverMetrics<SimpleScore>,
    after: bool,
    previous: &mut Option<(SimpleScore, bool)>,
    count: &mut usize,
) {
    if let SolverEvent::Best(u) = event {
        let snapshot = m.snapshot();
        let live = &snapshot.active[0];
        let expected = if after {
            *previous
        } else {
            Some((u.score, u.complete))
        };
        assert_eq!(live.best_score, expected.map(|x| x.0));
        assert_eq!(live.complete, expected.is_some_and(|x| x.1));
        if let Some((score, _)) = expected {
            let BestConstraintTotals::Available(totals) = &live.best_constraint_totals else {
                panic!("published score needs totals");
            };
            assert_eq!(totals["value"].score.score(), score.score());
        } else {
            assert!(!matches!(
                live.best_constraint_totals,
                BestConstraintTotals::Available(_)
            ));
        }
        *previous = Some((u.score, u.complete));
        *count += 1;
    }
}
#[test]
fn ordering_and_totals_align_in_scoped_fresh_and_retained_stream_plans_without_rescoring() {
    let p = basic();
    for backend in 0..3 {
        for after in [false, true] {
            let mut m = SolverMetrics::default().with_best_constraint_totals();
            if after {
                m = m.with_best_metrics_after_callbacks();
            }
            let calls = Cell::new(0);
            let mut previous = None;
            let mut count = 0;
            let mut events = |e: SolverEvent<'_, Assignment<i32>, SimpleScore>| {
                check(e, &m, after, &mut previous, &mut count)
            };
            macro_rules! extensions {
                () => {
                    SolverExtensions {
                        metrics: Some(&m),
                        events: Some(&mut events),
                        ..Default::default()
                    }
                };
            }
            let r = match backend {
                0 => {
                    solve_with_extensions(&p, &network(&calls), sequence(), extensions!()).unwrap()
                }
                1 => solve_stream_with_extensions(
                    &p,
                    || Ok(graph(&calls)),
                    sequence(),
                    extensions!(),
                )
                .unwrap(),
                _ => {
                    let (g, s) = graph(&calls);
                    StreamScoreDirector::new(&p, g, s, p.initial_assignment())
                        .unwrap()
                        .run_solver_with_extensions(sequence(), extensions!())
                        .unwrap()
                }
            };
            assert!(count >= 3);
            assert_eq!(r.score, Some(score(2)));
            let live = m.snapshot().latest.unwrap();
            assert_eq!(live.best_score, r.score);
            assert_eq!(live.complete, r.complete);
            let observed_calls = calls.get();
            calls.set(0);
            let plain = match backend {
                0 => solve(&p, &network(&calls), sequence()).unwrap(),
                1 => solve_stream(&p, || Ok(graph(&calls)), sequence()).unwrap(),
                _ => {
                    let (g, s) = graph(&calls);
                    StreamScoreDirector::new(&p, g, s, p.initial_assignment())
                        .unwrap()
                        .run_solver(sequence())
                        .unwrap()
                }
            };
            assert_eq!(r, plain);
            assert_eq!(observed_calls, calls.get());
        }
    }
}
#[test]
fn after_callback_panic_retains_the_preceding_best_and_totals_without_poisoning_monitor() {
    let p = basic();
    let calls = Cell::new(0);
    let n = network(&calls);
    for after in [false, true] {
        let mut m = SolverMetrics::default().with_best_constraint_totals();
        if after {
            m = m.with_best_metrics_after_callbacks();
        }
        let mut events = |event: SolverEvent<'_, Assignment<i32>, SimpleScore>| {
            if let SolverEvent::Best(u) = event {
                assert_ne!(u.score, score(1), "listener failure");
            }
        };
        assert!(
            catch_unwind(AssertUnwindSafe(|| solve_with_extensions(
                &p,
                &n,
                sequence(),
                SolverExtensions {
                    metrics: Some(&m),
                    events: Some(&mut events),
                    ..Default::default()
                }
            )))
            .is_err()
        );
        let snapshot = m.snapshot();
        assert!(snapshot.active.is_empty());
        assert_eq!(snapshot.failed_runs, 1);
        let last = snapshot.latest.unwrap();
        let expected = score(if after { 0 } else { 1 });
        assert_eq!(last.best_score, Some(expected));
        assert_eq!(last.outcome, Some(SolveMetricOutcome::Panicked));
        let BestConstraintTotals::Available(totals) = last.best_constraint_totals else {
            panic!()
        };
        assert_eq!(totals["value"].score, expected);
        let result = solve_with_extensions(
            &p,
            &n,
            sequence(),
            SolverExtensions {
                metrics: Some(&m),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.score, Some(score(2)));
        assert_eq!(m.snapshot().completed_runs, 2);
    }
}
#[test]
fn settings_belong_to_handles_and_prior_clones_keep_before_callback_delivery() {
    let p = basic();
    let calls = Cell::new(0);
    let n = network(&calls);
    let before = SolverMetrics::default().with_best_constraint_totals();
    let after = before.clone().with_best_metrics_after_callbacks();
    for (m, late) in [(&before, false), (&after, true), (&before, false)] {
        let mut previous = None;
        let mut count = 0;
        let mut events = |e: SolverEvent<'_, Assignment<i32>, SimpleScore>| {
            check(e, m, late, &mut previous, &mut count)
        };
        solve_with_extensions(
            &p,
            &n,
            sequence(),
            SolverExtensions {
                metrics: Some(m),
                events: Some(&mut events),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(count >= 3);
    }
    assert_eq!(before.snapshot().completed_runs, 3);
    assert_eq!(after.snapshot().completed_runs, 3);
}
#[test]
fn callback_free_after_order_matches_all_twelve_model_backend_paths() {
    use metrics_support::*;
    let bp = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 3]).unwrap();
    let lp = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![]); 2]).unwrap();
    let mp = MixedProblem::new(bp.clone(), lp.clone());
    let metrics = SolverMetrics::default().with_best_metrics_after_callbacks();
    for backend in 0..4 {
        macro_rules! compare {
            ($observed:expr,$plain:expr) => {{
                let r = $observed;
                assert_eq!(r, $plain);
                let s = metrics.snapshot().latest.unwrap();
                assert_eq!(s.best_score, r.score);
                assert_eq!(s.complete, r.complete);
                assert_eq!(s.work, r.work);
            }};
        }
        let config = SolverConfig {
            local_search: vec![LocalSearchConfig {
                max_steps: 3,
                ..Default::default()
            }],
            ..Default::default()
        };
        compare!(
            basic_run(&bp, backend, config.clone(), None, Some(&metrics)),
            basic_run(&bp, backend, config, None, None)
        );
        compare!(
            list_run(&lp, backend, None, Some(&metrics)),
            list_run(&lp, backend, None, None)
        );
        compare!(
            mixed_run(&mp, backend, None, Some(&metrics)),
            mixed_run(&mp, backend, None, None)
        );
    }
    assert_eq!(metrics.snapshot().completed_runs, 12);
}
#[test]
fn list_and_mixed_callbacks_observe_the_same_order_and_matched_constraint_totals() {
    let lists = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![])]).unwrap();
    let mixed = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![0]).with_initial(0).with_pinned(true),
        ])
        .unwrap(),
        lists.clone(),
    );
    for after in [false, true] {
        for backend in 0..3 {
            let mut metrics = SolverMetrics::default().with_best_constraint_totals();
            if after {
                metrics = metrics.with_best_metrics_after_callbacks();
            }
            let mut previous = None;
            let mut count = 0;
            let mut events = |e: SolverEvent<'_, ListAssignment<i32>, SimpleScore>| {
                check(e, &metrics, after, &mut previous, &mut count)
            };
            macro_rules! ext {
                ($event:expr) => {
                    SolverExtensions {
                        metrics: Some(&metrics),
                        events: Some($event),
                        ..Default::default()
                    }
                };
            }
            let config = || SolverSequence {
                phases: vec![SolverPhase::Construction(ConstructionConfig::default())],
                ..Default::default()
            };
            let graph = || {
                let mut b = StreamGraphBuilder::new();
                let s = ListStreamSources::<i32>::new(&mut b);
                b.score("value", s.entities.port(), |(_, row)| {
                    score(row.len() as i64)
                })
                .unwrap();
                (b.build(), s)
            };
            let result = match backend {
                0 => {
                    let mut n = ListConstraintNetwork::new(1, 2);
                    n.add_constraint("value", vec![ListScope::Entity(0)], |facts| {
                        let [ListFact::Entity(row)] = facts else {
                            panic!()
                        };
                        score(row.len() as i64)
                    })
                    .unwrap();
                    solve_list_with_extensions(&lists, &n, config(), ext!(&mut events)).unwrap()
                }
                1 => solve_list_stream_with_extensions(
                    &lists,
                    || Ok(graph()),
                    config(),
                    ext!(&mut events),
                )
                .unwrap(),
                _ => {
                    let (g, s) = graph();
                    ListStreamScoreDirector::new(&lists, g, s, lists.initial_assignment())
                        .unwrap()
                        .run_solver_with_extensions(config(), ext!(&mut events))
                        .unwrap()
                }
            };
            assert!(count > 1);
            assert_eq!(result.score, Some(score(2)));
            assert_eq!(metrics.snapshot().latest.unwrap().best_score, result.score);
            let mut previous = None;
            let mut count = 0;
            let mut events = |e: SolverEvent<'_, MixedAssignment<i32, i32>, SimpleScore>| {
                check(e, &metrics, after, &mut previous, &mut count)
            };
            let config = || MixedSolverSequence {
                phases: vec![MixedSolverPhase::Construction(
                    MixedConstructionConfig::default(),
                )],
                ..Default::default()
            };
            let graph = || {
                let mut b = StreamGraphBuilder::new();
                let s = MixedStreamSources::<i32, i32>::new(&mut b);
                b.score("value", s.lists.entities.port(), |(_, row)| {
                    score(row.len() as i64)
                })
                .unwrap();
                (b.build(), s)
            };
            let result = match backend {
                0 => {
                    let mut n = MixedConstraintNetwork::new(1, 1, 2);
                    n.add_constraint("value", vec![MixedScope::ListEntity(0)], |facts| {
                        let [MixedFact::ListEntity(row)] = facts else {
                            panic!()
                        };
                        score(row.len() as i64)
                    })
                    .unwrap();
                    solve_mixed_with_extensions(&mixed, &n, config(), ext!(&mut events)).unwrap()
                }
                1 => solve_mixed_stream_with_extensions(
                    &mixed,
                    || Ok(graph()),
                    config(),
                    ext!(&mut events),
                )
                .unwrap(),
                _ => {
                    let (g, s) = graph();
                    MixedStreamScoreDirector::new(&mixed, g, s, mixed.initial_assignment())
                        .unwrap()
                        .run_solver_with_extensions(config(), ext!(&mut events))
                        .unwrap()
                }
            };
            assert!(count > 1);
            assert_eq!(result.score, Some(score(2)));
            assert_eq!(metrics.snapshot().latest.unwrap().best_score, result.score);
        }
    }
}
