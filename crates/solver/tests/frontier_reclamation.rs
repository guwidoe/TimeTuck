//! Original capacity-pressure regressions; no upstream-method credit.
use std::cell::Cell;
use timetuck::*;
const ORDERS: [NodeExploration; 4] = [
    NodeExploration::DepthFirst,
    NodeExploration::BreadthFirst,
    NodeExploration::ScoreFirst,
    NodeExploration::OptimisticBoundFirst,
];
fn problem() -> FiniteProblem<i64> {
    FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2]),
        Variable::new(vec![0, 1, 2]),
        Variable::new(vec![0]),
    ])
    .unwrap()
}
// Every partial score bounds its descendants. The first branch finds 20,
// leaving two score-10 branches and a score-0 branch behind the score-50
// branch. Expanding that branch needs room for three score-40 children.
fn objective(first: Option<i64>, second: Option<i64>, complete: bool) -> SimpleScore {
    SimpleScore::new(match (first, second, complete) {
        (None, _, _) => 1000,
        (Some(0), None, _) => 100,
        (Some(0), Some(0), false) => 100,
        (Some(0), Some(0), true) => 20,
        (Some(0), Some(_), _) => 10,
        (Some(1), None, _) => 50,
        (Some(1), Some(_), _) => 40,
        _ => 0,
    })
}
fn basic_score(a: &Assignment<i64>) -> SimpleScore {
    objective(a.values()[0], a.values()[1], a.values()[2].is_some())
}
fn config(order: NodeExploration, capacity: usize, budget: u64) -> BranchAndBoundConfig<'static> {
    BranchAndBoundConfig {
        exploration: order,
        max_frontier: capacity,
        max_evaluations: budget,
        ..Default::default()
    }
}
#[test]
fn dead_frontier_entries_no_longer_hide_a_better_solution() {
    let p = problem();
    for order in [
        NodeExploration::ScoreFirst,
        NodeExploration::OptimisticBoundFirst,
    ] {
        let mut baseline_trace = Vec::new();
        let baseline = branch_and_bound(
            &p,
            |a, _| {
                baseline_trace.push(a.clone());
                BranchEvaluation::bounded(basic_score(a), basic_score(a))
            },
            config(order, 100, 100),
        )
        .unwrap();
        assert_eq!(baseline.peak_frontier, 6);
        let mut trace = Vec::new();
        let mut observed = Vec::new();
        let r = branch_and_bound_with_observer(
            &p,
            |a, _| {
                trace.push(a.clone());
                BranchEvaluation::bounded(basic_score(a), basic_score(a))
            },
            config(order, 5, 100),
            |best| observed.push(best.clone()),
        )
        .unwrap();
        assert!(r.is_optimal());
        assert_eq!(r.best.as_ref().unwrap().score, SimpleScore::new(40));
        assert_eq!(r.peak_frontier, 5);
        assert_eq!(r.best, baseline.best);
        assert_eq!(r.evaluations, baseline.evaluations);
        assert_eq!(r.complete_evaluations, 2);
        assert_eq!(r.pruned_branches, baseline.pruned_branches);
        assert_eq!(r.expanded_nodes, baseline.expanded_nodes);
        assert_eq!(trace, baseline_trace);
        assert_eq!(
            observed.iter().map(|b| b.score).collect::<Vec<_>>(),
            vec![SimpleScore::new(20), SimpleScore::new(40)]
        );
        assert_eq!(observed.last(), r.best.as_ref());
    }
}
#[test]
fn every_capacity_and_budget_preserves_the_scheduling_prefix_and_best_leaf() {
    let p = problem();
    for order in ORDERS {
        for budget in 0..=25 {
            let mut expected = Vec::new();
            let baseline = branch_and_bound(
                &p,
                |a, _| {
                    expected.push(a.clone());
                    BranchEvaluation::bounded(basic_score(a), basic_score(a))
                },
                config(order, 100, budget),
            )
            .unwrap();
            for capacity in 0..=8 {
                let mut trace = Vec::new();
                let mut best = None;
                let mut best_score = None;
                let r = branch_and_bound(
                    &p,
                    |a, left| {
                        trace.push(a.clone());
                        let score = basic_score(a);
                        if left.is_empty() && best_score.is_none_or(|s| score > s) {
                            best = Some(a.clone());
                            best_score = Some(score);
                        }
                        BranchEvaluation::bounded(score, score)
                    },
                    config(order, capacity, budget),
                )
                .unwrap();
                assert_eq!(
                    trace,
                    expected[..trace.len()],
                    "{order:?}, cap {capacity}, budget {budget}"
                );
                assert_eq!(r.evaluations as usize, trace.len());
                assert_eq!(r.best.as_ref().map(|r| &r.assignment), best.as_ref());
                assert!(r.peak_frontier <= capacity);
                if r.termination != BranchTermination::FrontierLimit {
                    assert_eq!(r.best, baseline.best);
                    assert_eq!(r.termination, baseline.termination);
                    assert_eq!(r.evaluations, baseline.evaluations);
                    if r.is_optimal() {
                        assert_eq!(r.pruned_branches, baseline.pruned_branches);
                    }
                } else {
                    assert!(!r.is_optimal());
                }
            }
        }
    }
}
#[test]
fn unknown_bounds_survive_reclamation_and_real_capacity_pressure_still_stops() {
    let p = problem();
    // No finite ancestor bound: the third root branch is unknown even though
    // its partial scheduling score is lower than the retained complete score.
    let run = |capacity, unknown| {
        branch_and_bound(
            &p,
            |a, left| {
                let score = basic_score(a);
                if left.len() == 3 || (unknown && a.values()[0] == Some(2)) {
                    BranchEvaluation::unbounded(score)
                } else {
                    BranchEvaluation::bounded(score, score)
                }
            },
            config(NodeExploration::ScoreFirst, capacity, 100),
        )
        .unwrap()
    };
    let finite = run(5, false);
    let unknown = run(5, true);
    assert!(finite.is_optimal());
    assert!(unknown.is_optimal());
    assert!(unknown.evaluations > finite.evaluations);
    assert_eq!(unknown.best, finite.best);
    // If every branch is unknown, capacity alone cannot authorize removing it.
    let mut observed = Vec::new();
    let r = branch_and_bound_with_observer(
        &p,
        |a, _| BranchEvaluation::unbounded(basic_score(a)),
        config(NodeExploration::ScoreFirst, 5, 100),
        |b| observed.push(b.clone()),
    )
    .unwrap();
    assert_eq!(r.termination, BranchTermination::FrontierLimit);
    assert_eq!(r.best.as_ref().unwrap().score, SimpleScore::new(20));
    assert_eq!(observed.last(), r.best.as_ref());
    assert_eq!(r.pruned_branches, 0);
}
#[test]
fn observer_cancellation_and_invalid_bounds_preserve_the_existing_contract() {
    let p = problem();
    for target in [20, 40] {
        let token = CancellationToken::new();
        let mut c = config(NodeExploration::ScoreFirst, 5, 100);
        c.cancellation = Some(&token);
        let r = branch_and_bound_with_observer(
            &p,
            |a, _| BranchEvaluation::bounded(basic_score(a), basic_score(a)),
            c,
            |b| {
                if b.score == SimpleScore::new(target) {
                    token.cancel();
                }
            },
        )
        .unwrap();
        assert_eq!(r.best.as_ref().unwrap().score, SimpleScore::new(target));
        assert_eq!(
            r.termination,
            if target == 20 {
                BranchTermination::Cancelled
            } else {
                BranchTermination::Exhausted
            }
        );
    }
    let r = branch_and_bound(
        &p,
        |a, left| {
            let mut score = basic_score(a);
            if left.is_empty() && a.values()[0] == Some(1) {
                score = SimpleScore::new(41);
            }
            BranchEvaluation::bounded(score, basic_score(a))
        },
        config(NodeExploration::ScoreFirst, 5, 100),
    );
    assert!(r.is_err()); // Reclaimed storage does not weaken visited-leaf validation.
}
fn list_problem() -> ListProblem<i64> {
    ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![]); 3]).unwrap()
}
fn owner(a: &ListAssignment<i64>, v: i64) -> Option<i64> {
    a.element(&v).map(|e| e.position.entity as i64)
}
fn list_score(a: &ListAssignment<i64>) -> SimpleScore {
    objective(owner(a, 0), owner(a, 1), owner(a, 2).is_some())
}
fn mixed_problem() -> MixedProblem<i64, i64> {
    MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1, 2])]).unwrap(),
        ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![]); 3]).unwrap(),
    )
}
fn mixed_score(a: &MixedAssignment<i64, i64>) -> SimpleScore {
    objective(
        owner(a.lists(), 0),
        owner(a.lists(), 1),
        a.basic().values()[0].is_some(),
    )
}
#[test]
fn list_and_mixed_frontiers_reclaim_without_changing_callback_order() {
    for order in ORDERS {
        for budget in [0, 8, 20, 1000] {
            let mut generous_trace = Vec::new();
            let p = list_problem();
            let generous = branch_and_bound_lists(
                &p,
                |a, _| {
                    generous_trace.push(a.clone());
                    BranchEvaluation::bounded(list_score(a), list_score(a))
                },
                config(order, 1000, budget),
            )
            .unwrap();
            let mut trace = Vec::new();
            let limited = branch_and_bound_lists(
                &p,
                |a, _| {
                    trace.push(a.clone());
                    BranchEvaluation::bounded(list_score(a), list_score(a))
                },
                config(order, 6, budget),
            )
            .unwrap();
            assert_eq!(trace, generous_trace[..trace.len()]);
            if order != NodeExploration::BreadthFirst {
                assert_eq!(limited.best, generous.best);
                assert_eq!(limited.termination, generous.termination);
            }
            if matches!(
                order,
                NodeExploration::ScoreFirst | NodeExploration::OptimisticBoundFirst
            ) && budget == 1000
            {
                assert!(limited.is_optimal());
                assert_eq!(limited.best.unwrap().score, SimpleScore::new(40));
                assert!(generous.peak_frontier > limited.peak_frontier);
            }
            let p = mixed_problem();
            let mut generous_trace = Vec::new();
            let generous = branch_and_bound_mixed(
                &p,
                |a, _, _| {
                    generous_trace.push(a.clone());
                    BranchEvaluation::bounded(mixed_score(a), mixed_score(a))
                },
                config(order, 1000, budget),
            )
            .unwrap();
            let mut trace = Vec::new();
            let limited = branch_and_bound_mixed(
                &p,
                |a, _, _| {
                    trace.push(a.clone());
                    BranchEvaluation::bounded(mixed_score(a), mixed_score(a))
                },
                config(order, 6, budget),
            )
            .unwrap();
            assert_eq!(trace, generous_trace[..trace.len()]);
            if order != NodeExploration::BreadthFirst {
                assert_eq!(limited.best, generous.best);
                assert_eq!(limited.termination, generous.termination);
            }
            if matches!(
                order,
                NodeExploration::ScoreFirst | NodeExploration::OptimisticBoundFirst
            ) && budget == 1000
            {
                assert!(limited.is_optimal());
                assert_eq!(limited.best.unwrap().score, SimpleScore::new(40));
                assert!(generous.peak_frontier > limited.peak_frontier);
            }
        }
    }
}

#[derive(Clone)]
struct Calculator<'a, A, C> {
    score: fn(&A) -> SimpleScore,
    current: SimpleScore,
    calls: &'a Cell<u128>,
    change: std::marker::PhantomData<C>,
}
impl<'a, A, C> Calculator<'a, A, C> {
    fn new(score: fn(&A) -> SimpleScore, calls: &'a Cell<u128>) -> Self {
        Self {
            score,
            current: SimpleScore::ZERO,
            calls,
            change: Default::default(),
        }
    }
}
impl<A, C> IncrementalCalculator<A> for Calculator<'_, A, C> {
    type Score = SimpleScore;
    type Justification = ();
    type Change = C;
    fn reset(
        &mut self,
        a: &A,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.current = (self.score)(a);
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.calls.set(self.calls.get() + 1);
        self.current
    }
}
fn sequence(order: NodeExploration, cap: usize, budget: u64) -> SolverSequence<'static> {
    SolverSequence {
        limits: SolverLimits {
            max_evaluations: budget,
            ..Default::default()
        },
        phases: vec![SolverPhase::Exact(ExactPhaseConfig {
            strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig {
                exploration: order,
                max_frontier: cap,
                bound: PartialScoreBound::NonIncreasing,
            }),
            ..Default::default()
        })],
    }
}
fn mixed_sequence(c: SolverSequence<'static>) -> MixedSolverSequence<'static> {
    MixedSolverSequence {
        limits: c.limits,
        phases: c
            .phases
            .into_iter()
            .map(|p| {
                let SolverPhase::Exact(c) = p else {
                    unreachable!()
                };
                MixedSolverPhase::Exact(c)
            })
            .collect(),
    }
}
fn same<A: std::fmt::Debug + Eq>(
    mut a: SolverResult<A, SimpleScore>,
    mut b: SolverResult<A, SimpleScore>,
) {
    a.work.constraint_evaluations = 0;
    b.work.constraint_evaluations = 0;
    for p in &mut a.phases {
        p.work.constraint_evaluations = 0;
    }
    for p in &mut b.phases {
        p.work.constraint_evaluations = 0;
    }
    assert_eq!(a, b);
}
#[test]
fn solver_phases_keep_best_snapshots_and_charge_only_actual_calculator_work() {
    for order in ORDERS {
        for budget in [0, 8, 11, 20, 1000] {
            let p = problem();
            let calls = Cell::new(0);
            let factory = || Calculator::<_, Vec<usize>>::new(basic_score, &calls);
            let c = sequence(order, 5, budget);
            let expected = solve_calculator(&p, &factory, c.clone()).unwrap();
            assert_eq!(expected.work.constraint_evaluations, calls.get());
            let mut d =
                CalculatorScoreDirector::new(&p, p.initial_assignment(), factory, false).unwrap();
            let before = calls.get();
            let actual = d.run_solver(c).unwrap();
            assert_eq!(actual.work.constraint_evaluations, calls.get() - before);
            if let Some(score) = actual.score {
                assert_eq!(d.score().unwrap(), score);
                assert_eq!(d.assignment(), &actual.assignment);
            }
            same(actual, expected.clone());
            if matches!(
                order,
                NodeExploration::ScoreFirst | NodeExploration::OptimisticBoundFirst
            ) && budget == 1000
            {
                assert_eq!(expected.termination, SolverTermination::Optimal);
                assert_eq!(expected.score, Some(SimpleScore::new(40)));
            }
            let p = list_problem();
            let calls = Cell::new(0);
            let factory = || Calculator::<_, Vec<usize>>::new(list_score, &calls);
            let c = sequence(order, 6, budget);
            let expected = solve_list_calculator(&p, &factory, c.clone()).unwrap();
            assert_eq!(expected.work.constraint_evaluations, calls.get());
            let mut d =
                ListCalculatorScoreDirector::new(&p, p.initial_assignment(), factory, false)
                    .unwrap();
            let before = calls.get();
            let actual = d.run_solver(c).unwrap();
            assert_eq!(actual.work.constraint_evaluations, calls.get() - before);
            if let Some(score) = actual.score {
                assert_eq!(d.score().unwrap(), score);
                assert_eq!(d.assignment(), &actual.assignment);
            }
            same(actual, expected.clone());
            if matches!(
                order,
                NodeExploration::ScoreFirst | NodeExploration::OptimisticBoundFirst
            ) && budget == 1000
            {
                assert_eq!(expected.termination, SolverTermination::Optimal);
                assert_eq!(expected.score, Some(SimpleScore::new(40)));
            }
            let p = mixed_problem();
            let calls = Cell::new(0);
            let factory = || Calculator::<_, MixedCalculatorChange>::new(mixed_score, &calls);
            let c = mixed_sequence(sequence(order, 6, budget));
            let expected = solve_mixed_calculator(&p, &factory, c.clone()).unwrap();
            assert_eq!(expected.work.constraint_evaluations, calls.get());
            let mut d =
                MixedCalculatorScoreDirector::new(&p, p.initial_assignment(), factory, false)
                    .unwrap();
            let before = calls.get();
            let actual = d.run_solver(c).unwrap();
            assert_eq!(actual.work.constraint_evaluations, calls.get() - before);
            if let Some(score) = actual.score {
                assert_eq!(d.score().unwrap(), score);
                assert_eq!(d.assignment(), &actual.assignment);
            }
            same(actual, expected.clone());
            if matches!(
                order,
                NodeExploration::ScoreFirst | NodeExploration::OptimisticBoundFirst
            ) && budget == 1000
            {
                assert_eq!(expected.termination, SolverTermination::Optimal);
                assert_eq!(expected.score, Some(SimpleScore::new(40)));
            }
        }
    }
}
fn basic_graph()
-> Result<(StreamGraph<'static, SimpleScore>, PlanningStreamSource<i64>), GraphError> {
    let mut b = StreamGraphBuilder::new();
    let s = b.source::<usize, (usize, Option<i64>)>();
    let x = b.filter(s.port(), |r| r.0 == 0)?;
    let y = b.filter(s.port(), |r| r.0 == 1)?;
    let z = b.filter(s.port(), |r| r.0 == 2)?;
    let xy = b.join_equal(&x, &y, |_| (), |_| ())?;
    let xyz = b.join_equal(&xy, &z, |_| (), |_| ())?;
    b.score("bound", &xyz, |((x, y), z)| {
        objective(x.1, y.1, z.1.is_some())
    })?;
    Ok((b.build(), s))
}
fn list_graph() -> Result<(StreamGraph<'static, SimpleScore>, ListStreamSources<i64>), GraphError> {
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::new(&mut b);
    let x = b.filter(s.values.port(), |r| r.range_index == 0)?;
    let y = b.filter(s.values.port(), |r| r.range_index == 1)?;
    let z = b.filter(s.values.port(), |r| r.range_index == 2)?;
    let xy = b.join_equal(&x, &y, |_| (), |_| ())?;
    let xyz = b.join_equal(&xy, &z, |_| (), |_| ())?;
    b.score("bound", &xyz, |((x, y), z)| {
        objective(
            x.position.map(|p| p.entity as i64),
            y.position.map(|p| p.entity as i64),
            z.position.is_some(),
        )
    })?;
    Ok((b.build(), s))
}
fn mixed_graph() -> Result<
    (
        StreamGraph<'static, SimpleScore>,
        MixedStreamSources<i64, i64>,
    ),
    GraphError,
> {
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::new(&mut b);
    let y = b.filter(s.lists.values.port(), |r| r.range_index == 0)?;
    let z = b.filter(s.lists.values.port(), |r| r.range_index == 1)?;
    let xy = b.join_equal(s.basic.port(), &y, |_| (), |_| ())?;
    let xyz = b.join_equal(&xy, &z, |_| (), |_| ())?;
    b.score("bound", &xyz, |((x, y), z)| {
        objective(
            y.position.map(|p| p.entity as i64),
            z.position.map(|p| p.entity as i64),
            x.1.is_some(),
        )
    })?;
    Ok((b.build(), s))
}
#[test]
fn scoped_factory_stream_and_retained_stream_phases_share_reclamation() {
    for order in [
        NodeExploration::ScoreFirst,
        NodeExploration::OptimisticBoundFirst,
    ] {
        for budget in [8, 20, 1000] {
            let p = problem();
            let mut n = ConstraintNetwork::new(3);
            n.add_constraint("bound", vec![0, 1, 2], |v| {
                objective(v[0].copied(), v[1].copied(), v[2].is_some())
            })
            .unwrap();
            let c = sequence(order, 5, budget);
            let expected = solve(&p, &n, c.clone()).unwrap();
            same(
                solve_stream(&p, basic_graph, c.clone()).unwrap(),
                expected.clone(),
            );
            let (g, s) = basic_graph().unwrap();
            let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
            let r = d.run_solver(c).unwrap();
            same(r.clone(), expected);
            assert_eq!(d.assignment(), &r.assignment);
            if let Some(score) = r.score {
                assert_eq!(d.score(), score);
            }
            if budget == 1000 {
                assert_eq!(r.termination, SolverTermination::Optimal);
            }

            let p = list_problem();
            let mut n = ListConstraintNetwork::new(3, 3);
            n.add_constraint(
                "bound",
                vec![
                    ListScope::Value(0),
                    ListScope::Value(1),
                    ListScope::Value(2),
                ],
                |v| {
                    let positions: Vec<_> = v
                        .iter()
                        .map(|f| {
                            let ListFact::Value { element, .. } = f else {
                                unreachable!()
                            };
                            element.as_ref().map(|e| e.position.entity as i64)
                        })
                        .collect();
                    objective(positions[0], positions[1], positions[2].is_some())
                },
            )
            .unwrap();
            let c = sequence(order, 6, budget);
            let expected = solve_list(&p, &n, c.clone()).unwrap();
            same(
                solve_list_stream(&p, list_graph, c.clone()).unwrap(),
                expected.clone(),
            );
            let (g, s) = list_graph().unwrap();
            let mut d = ListStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
            let r = d.run_solver(c).unwrap();
            same(r.clone(), expected);
            assert_eq!(d.assignment(), &r.assignment);
            if let Some(score) = r.score {
                assert_eq!(d.score(), score);
            }
            if budget == 1000 {
                assert_eq!(r.termination, SolverTermination::Optimal);
            }

            let p = mixed_problem();
            let mut n = MixedConstraintNetwork::new(1, 3, 2);
            n.add_constraint(
                "bound",
                vec![
                    MixedScope::Basic(0),
                    MixedScope::ListValue(0),
                    MixedScope::ListValue(1),
                ],
                |v| {
                    let MixedFact::Basic(x) = &v[0] else {
                        unreachable!()
                    };
                    let MixedFact::ListValue { element: y, .. } = &v[1] else {
                        unreachable!()
                    };
                    let MixedFact::ListValue { element: z, .. } = &v[2] else {
                        unreachable!()
                    };
                    objective(
                        y.as_ref().map(|e| e.position.entity as i64),
                        z.as_ref().map(|e| e.position.entity as i64),
                        x.is_some(),
                    )
                },
            )
            .unwrap();
            let c = mixed_sequence(sequence(order, 6, budget));
            let expected = solve_mixed(&p, &n, c.clone()).unwrap();
            same(
                solve_mixed_stream(&p, mixed_graph, c.clone()).unwrap(),
                expected.clone(),
            );
            let (g, s) = mixed_graph().unwrap();
            let mut d = MixedStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
            let r = d.run_solver(c).unwrap();
            same(r.clone(), expected);
            assert_eq!(d.assignment(), &r.assignment);
            if let Some(score) = r.score {
                assert_eq!(d.score(), score);
            }
            if budget == 1000 {
                assert_eq!(r.termination, SolverTermination::Optimal);
            }
        }
    }
}
#[test]
fn equal_and_inherited_bounds_are_reclaimed_using_full_multilevel_order() {
    let p = problem();
    let score = |s: SimpleScore| match s.score() {
        0 => HardSoftScore::new(0, 0),
        10 => HardSoftScore::new(0, 10),
        20 => HardSoftScore::new(0, 20),
        40 => HardSoftScore::new(1, -100),
        50 => HardSoftScore::new(1, 0),
        100 => HardSoftScore::new(2, 0),
        _ => HardSoftScore::new(3, 0),
    };
    let evaluate = |a: &Assignment<i64>, _left: &[usize]| {
        let ordinary = basic_score(a);
        let bound = if ordinary == SimpleScore::new(10) {
            SimpleScore::new(20)
        } else {
            ordinary
        };
        BranchEvaluation::bounded(score(ordinary), score(bound))
    };
    let generous =
        branch_and_bound(&p, evaluate, config(NodeExploration::ScoreFirst, 100, 100)).unwrap();
    let limited =
        branch_and_bound(&p, evaluate, config(NodeExploration::ScoreFirst, 5, 100)).unwrap();
    assert!(limited.is_optimal());
    assert_eq!(
        limited.best.as_ref().unwrap().score,
        HardSoftScore::new(1, -100)
    );
    assert_eq!(limited.best, generous.best);
    assert_eq!(limited.evaluations, generous.evaluations);
    assert_eq!(limited.pruned_branches, generous.pruned_branches);
}
