//! Original generated-stream transition regressions; no upstream source credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[derive(Clone, Debug)]
struct Value {
    id: u8,
    payload: i64,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
fn v(id: u8, payload: i64) -> Value {
    Value { id, payload }
}
fn payloads(a: &Assignment<Value>) -> Vec<Option<i64>> {
    a.values()
        .iter()
        .map(|v| v.as_ref().map(|v| v.payload))
        .collect()
}
fn problem() -> FiniteProblem<Value> {
    FiniteProblem::new(vec![
        Variable::new(vec![v(0, 2), v(1, 4)]).with_initial(v(0, 90)),
        Variable::new(vec![v(0, 3), v(1, 6)]).allowing_unassigned(),
        Variable::new(vec![v(0, 11)])
            .with_initial(v(0, 111))
            .with_pinned(true),
        Variable::new(vec![v(0, 5), v(1, 10)]),
    ])
    .unwrap()
}
fn graph(calls: &Cell<u128>) -> (StreamGraph<'_, SimpleScore>, PlanningStreamSource<Value>) {
    let mut g = StreamGraphBuilder::new();
    let s = g.source::<usize, (usize, Option<Value>)>();
    g.score("payload", s.port(), |(i, v)| {
        calls.set(calls.get() + 1);
        SimpleScore::new((*i as i64 + 1) * v.as_ref().map_or(0, |v| v.payload))
    })
    .unwrap();
    let coupled = g
        .join_equal(s.port(), s.port(), |(i, _)| *i, |(i, _)| (*i + 1) % 4)
        .unwrap();
    g.score("neighbors", &coupled, |((_, a), (_, b))| {
        calls.set(calls.get() + 1);
        SimpleScore::new(a.as_ref().map_or(0, |v| v.payload) * b.as_ref().map_or(0, |v| v.payload))
    })
    .unwrap();
    (g.build(), s)
}
fn strategies() -> Vec<ExactSearchStrategy> {
    let mut all = vec![
        ExactSearchStrategy::Enumerate,
        ExactSearchStrategy::Complete,
    ];
    for exploration in [
        NodeExploration::DepthFirst,
        NodeExploration::BreadthFirst,
        NodeExploration::ScoreFirst,
        NodeExploration::OptimisticBoundFirst,
        NodeExploration::InputOrderDepthFirst,
    ] {
        let c = BoundedPhaseConfig {
            exploration,
            order: Some(
                BranchOrder {
                    basic_variables: Some(vec![3, 2, 1, 0]),
                    basic_values: Some(vec![vec![1, 0], vec![1, 0], vec![0], vec![1, 0]]),
                    ..Default::default()
                }
                .into(),
            ),
            ..Default::default()
        };
        all.push(ExactSearchStrategy::BranchAndBound(c.clone()));
        all.push(ExactSearchStrategy::CompleteBranchAndBound(c));
    }
    all
}
fn plan(strategy: ExactSearchStrategy, budget: u64) -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![SolverPhase::Exact(ExactPhaseConfig {
            strategy,
            max_evaluations: budget,
            ..Default::default()
        })],
        ..Default::default()
    }
}
#[test]
fn every_order_and_prefix_matches_full_replacement_oracle_and_fresh_scores() {
    let p = problem();
    let mut saved_work = 0;
    for strategy in strategies() {
        for budget in (0..=20).chain([1000]) {
            let c = plan(strategy.clone(), budget);
            let fresh_calls = Cell::new(0);
            let fresh = solve_stream(&p, || Ok(graph(&fresh_calls)), c.clone()).unwrap();
            let calls = Cell::new(0);
            let (g, s) = graph(&calls);
            let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
            let before = calls.get();
            let oracle_calls = Cell::new(0);
            let (g, s) = graph(&oracle_calls);
            let mut oracle = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
            let mut seen = 0;
            let mut full = |node: ExactBoundContext<'_, Assignment<Value>, SimpleScore>| {
                let update = oracle.replace_assignment(node.assignment.clone()).unwrap();
                assert_eq!(update.score(), *node.score);
                update.commit();
                seen += 1;
                None
            };
            let mut ext = SolverExtensions::default();
            if matches!(
                strategy,
                ExactSearchStrategy::BranchAndBound(_)
                    | ExactSearchStrategy::CompleteBranchAndBound(_)
            ) {
                ext.optimistic_bounds.bind(0, &mut full);
            }
            let r = d.run_solver_with_extensions(c, ext).unwrap();
            assert_eq!(
                payloads(&r.assignment),
                payloads(&fresh.assignment),
                "{strategy:?} budget{budget}"
            );
            assert_eq!(r.score, fresh.score);
            assert_eq!(r.complete, fresh.complete);
            assert_eq!(r.termination, fresh.termination);
            assert_eq!(r.work.evaluations, fresh.work.evaluations);
            assert_eq!(r.work.move_evaluations, fresh.work.move_evaluations);
            assert_eq!(r.work.completed_steps, fresh.work.completed_steps);
            assert_eq!(r.work.constraint_evaluations, calls.get() - before);
            if matches!(
                strategy,
                ExactSearchStrategy::BranchAndBound(_)
                    | ExactSearchStrategy::CompleteBranchAndBound(_)
            ) {
                assert_eq!(seen, r.work.evaluations);
            } else {
                // Independent complete visitor feeds the unchanged public replacement
                // path; completion keeps phase-entry payloads, enumeration clears only
                // movable values. Optional None is generated last by the visitor.
                let root = if strategy == ExactSearchStrategy::Complete {
                    p.initial_assignment()
                } else {
                    Assignment::new(
                        p.variables()
                            .iter()
                            .map(|v| v.is_pinned().then(|| v.initial().cloned()).flatten())
                            .collect(),
                    )
                };
                let mut best = None;
                visit_basic_completions(&p, &root, |a| {
                    if seen >= budget {
                        return false;
                    }
                    let change = oracle.replace_assignment(a.clone()).unwrap();
                    let score = change.score();
                    change.commit();
                    if score.structural() >= 0 && best.as_ref().is_none_or(|(s, _)| score > *s) {
                        best = Some((score, payloads(a)));
                    }
                    seen += 1;
                    true
                })
                .unwrap();
                assert_eq!(seen, r.work.evaluations);
                assert_eq!(best.as_ref().map(|x| x.0), r.score);
                if let Some((_, a)) = best {
                    assert_eq!(a, payloads(&r.assignment));
                }
            }
            assert_eq!(payloads(d.assignment()), payloads(&r.assignment));
            d.assert_consistent().unwrap();
            if budget == 1000 {
                assert!(r.work.constraint_evaluations <= fresh.work.constraint_evaluations);
                saved_work += fresh.work.constraint_evaluations - r.work.constraint_evaluations;
            }
        }
    }
    assert!(saved_work > 0);
}
#[test]
fn sparse_rows_reduce_real_callbacks_while_public_replacement_refreshes_equal_payloads() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]); 12]).unwrap();
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::new();
    let s = b.source::<usize, (usize, Option<i64>)>();
    b.score("row", s.port(), |(_, v)| {
        calls.set(calls.get() + 1);
        SimpleScore::new(v.unwrap_or(0))
    })
    .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    let initial = calls.get();
    let r = d
        .run_solver(plan(ExactSearchStrategy::Enumerate, 1000))
        .unwrap();
    assert_eq!(r.work.evaluations, 1000);
    assert_eq!(calls.get() - initial, r.work.constraint_evaluations);
    assert!(r.work.constraint_evaluations < 2100); // Full replacement makes12,000 callbacks.
    let public_before = calls.get();
    d.replace_assignment(d.assignment().clone())
        .unwrap()
        .commit();
    assert_eq!(calls.get() - public_before, 12);

    let p = FiniteProblem::new(vec![Variable::new(vec![v(0, 1)]).with_initial(v(0, 99))]).unwrap();
    let calls = Cell::new(0);
    let (g, s) = graph(&calls);
    let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    d.replace_assignment(Assignment::new(vec![Some(v(0, 7))]))
        .unwrap()
        .commit();
    assert_eq!(d.score(), SimpleScore::new(7));
    let r = d
        .run_solver(plan(ExactSearchStrategy::Enumerate, 10))
        .unwrap();
    assert_eq!(r.score, Some(SimpleScore::new(1)));
    assert_eq!(payloads(d.assignment()), vec![Some(1)]);
}
#[test]
fn repeated_phases_rearm_refresh_and_restore_prior_best_payloads() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![v(0, 1), v(1, 2)]).with_initial(v(0, 99)),
    ])
    .unwrap();
    for strategy in strategies().into_iter().take(2) {
        let calls = Cell::new(0);
        let (g, s) = graph(&calls);
        let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
        let c = SolverSequence {
            phases: vec![
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 0,
                    ..Default::default()
                }),
                SolverPhase::Exact(ExactPhaseConfig {
                    strategy: strategy.clone(),
                    max_evaluations: 1,
                    ..Default::default()
                }),
                SolverPhase::Exact(ExactPhaseConfig {
                    strategy,
                    max_evaluations: 1,
                    ..Default::default()
                }),
            ],
            ..Default::default()
        };
        let r = d.run_solver(c).unwrap();
        assert_eq!(r.score, Some(SimpleScore::new(99)));
        assert_eq!(payloads(d.assignment()), vec![Some(99)]);
        d.assert_consistent().unwrap();
        assert_eq!(r.phases.len(), 3);
    }
}
#[test]
fn callback_error_and_panic_roll_back_sparse_installation_and_allow_full_recovery() {
    for panic in [false, true] {
        let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]); 3]).unwrap();
        let fail = Cell::new(false);
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<i64>)>();
        b.score("failure", s.port(), |(i, v)| {
            if fail.get() && *i == 2 && *v == Some(1) {
                if panic {
                    panic!("sparse callback");
                }
                return SimpleScore::ZERO.with_structural(-1);
            }
            SimpleScore::new(v.unwrap_or(0))
        })
        .unwrap();
        let mut d = StreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
        fail.set(true);
        let result = catch_unwind(AssertUnwindSafe(|| {
            d.run_solver(plan(ExactSearchStrategy::Enumerate, 10))
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(d.assignment().values(), &[Some(0), Some(0), Some(0)]);
        assert_eq!(d.score(), SimpleScore::ZERO);
        fail.set(false);
        d.assert_consistent().unwrap();
        d.replace_assignment(Assignment::new(vec![Some(1); 3]))
            .unwrap()
            .commit();
        assert_eq!(d.score(), SimpleScore::new(3));
    }
}
#[test]
fn cancellation_after_scoring_restores_same_prefix_best() {
    let p = problem();
    for strategy in strategies().into_iter().skip(2) {
        for cutoff in [1, 2, 5, 12] {
            let token = CancellationToken::new();
            let calls = Cell::new(0);
            let (g, s) = graph(&calls);
            let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
            let seen = Cell::new(0);
            let mut bound = |_: ExactBoundContext<'_, Assignment<Value>, SimpleScore>| {
                seen.set(seen.get() + 1);
                if seen.get() == cutoff {
                    token.cancel();
                }
                None
            };
            let mut ext = SolverExtensions::default();
            ext.optimistic_bounds.bind(0, &mut bound);
            let mut c = plan(strategy.clone(), 1000);
            let SolverPhase::Exact(ref mut exact) = c.phases[0] else {
                unreachable!()
            };
            exact.cancellation = Some(&token);
            let r = d.run_solver_with_extensions(c, ext).unwrap();
            let fresh_calls = Cell::new(0);
            let expected = solve_stream(
                &p,
                || Ok(graph(&fresh_calls)),
                plan(strategy.clone(), cutoff),
            );
            // Use a separate ordinary fresh solve with the identical scored prefix.
            let expected = expected.unwrap();
            assert_eq!(r.score, expected.score);
            assert_eq!(payloads(&r.assignment), payloads(&expected.assignment));
            assert_eq!(r.work.evaluations, expected.work.evaluations);
            assert_eq!(payloads(d.assignment()), payloads(&r.assignment));
            d.assert_consistent().unwrap();
        }
    }
}
