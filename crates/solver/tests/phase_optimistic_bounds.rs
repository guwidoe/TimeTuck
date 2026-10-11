//! Original typed phase-bound contracts; no upstream source credit.
use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    time::Duration,
};
use timetuck::*;
const ORDERS: [NodeExploration; 5] = [
    NodeExploration::DepthFirst,
    NodeExploration::BreadthFirst,
    NodeExploration::ScoreFirst,
    NodeExploration::OptimisticBoundFirst,
    NodeExploration::InputOrderDepthFirst,
];
fn phase(order: NodeExploration, bound: PartialScoreBound) -> ExactPhaseConfig<'static> {
    ExactPhaseConfig {
        strategy: ExactSearchStrategy::CompleteBranchAndBound(BoundedPhaseConfig {
            exploration: order,
            bound,
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn plan(c: ExactPhaseConfig<'_>) -> SolverSequence<'_> {
    SolverSequence {
        phases: vec![SolverPhase::Exact(c)],
        ..Default::default()
    }
}
fn mixed_plan(c: ExactPhaseConfig<'_>) -> MixedSolverSequence<'_> {
    MixedSolverSequence {
        phases: vec![MixedSolverPhase::Exact(c)],
        ..Default::default()
    }
}
trait Objective {
    fn objective(&self) -> SimpleScore;
}
impl Objective for Assignment<i64> {
    fn objective(&self) -> SimpleScore {
        SimpleScore::new(-self.values().iter().flatten().sum::<i64>())
    }
}
impl Objective for ListAssignment<i64> {
    fn objective(&self) -> SimpleScore {
        SimpleScore::new(
            -self
                .lists()
                .iter()
                .enumerate()
                .map(|(e, l)| {
                    l.iter()
                        .enumerate()
                        .map(|(i, v)| (i + 1 + e * 5) as i64 * v)
                        .sum::<i64>()
                })
                .sum::<i64>(),
        )
    }
}
impl Objective for MixedAssignment<i64, i64> {
    fn objective(&self) -> SimpleScore {
        SimpleScore::new(self.basic().objective().score() + self.lists().objective().score())
    }
}
#[derive(Clone)]
struct Cached<C> {
    score: SimpleScore,
    marker: PhantomData<C>,
}
fn cached<C>() -> Cached<C> {
    Cached {
        score: SimpleScore::ZERO,
        marker: PhantomData,
    }
}
impl<A: Objective, C> IncrementalCalculator<A> for Cached<C> {
    type Score = SimpleScore;
    type Justification = ();
    type Change = C;
    fn reset(
        &mut self,
        a: &A,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.score = a.objective();
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &A,
        _: &C,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.reset(a, r)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.score
    }
}
fn basic_graph() -> (StreamGraph<'static, SimpleScore>, PlanningStreamSource<i64>) {
    let mut g = StreamGraphBuilder::new();
    let s = g.source::<usize, (usize, Option<i64>)>();
    g.score("basic", s.port(), |(_, v)| {
        SimpleScore::new(-v.unwrap_or(0))
    })
    .unwrap();
    (g.build(), s)
}
fn list_graph() -> (StreamGraph<'static, SimpleScore>, ListStreamSources<i64>) {
    let mut g = StreamGraphBuilder::new();
    let s = ListStreamSources::<i64>::new(&mut g);
    g.score("list", s.values.port(), |v| {
        SimpleScore::new(
            -v.position
                .map_or(0, |p| (p.index + 1 + p.entity * 5) as i64 * v.value),
        )
    })
    .unwrap();
    (g.build(), s)
}
fn mixed_graph() -> (
    StreamGraph<'static, SimpleScore>,
    MixedStreamSources<i64, i64>,
) {
    let mut g = StreamGraphBuilder::new();
    let s = MixedStreamSources::<i64, i64>::new(&mut g);
    g.score("basic", s.basic.port(), |(_, v)| {
        SimpleScore::new(-v.unwrap_or(0))
    })
    .unwrap();
    g.score("list", s.lists.values.port(), |v| {
        SimpleScore::new(
            -v.position
                .map_or(0, |p| (p.index + 1 + p.entity * 5) as i64 * v.value),
        )
    })
    .unwrap();
    (g.build(), s)
}
fn check<A: Objective + std::fmt::Debug + PartialEq>(
    r: &SolverResult<A, SimpleScore>,
    expected: &A,
    completion: bool,
) {
    assert_eq!(&r.assignment, expected);
    assert_eq!(r.score, Some(expected.objective()));
    assert_eq!(r.is_optimal(), !completion);
    assert_eq!(r.work.evaluations, r.work.move_evaluations);
}
#[test]
fn all_eighteen_routes_prune_without_rescoring_and_restore_same_optimum() {
    let b = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2]).with_initial(2),
        Variable::new(vec![0, 1]),
    ])
    .unwrap();
    let l = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![3, 1]), ListEntity::new(vec![])],
    )
    .unwrap();
    let m = MixedProblem::new(b.clone(), l.clone());
    for completion in [false, true] {
        let eb = Assignment::new(vec![Some(if completion { 2 } else { 0 }), Some(0)]);
        let el = ListAssignment::new(vec![vec![3, 2, 1], vec![]]);
        let em = MixedAssignment::new(eb.clone(), el.clone());
        let mut bn = ConstraintNetwork::new(2);
        bn.add_constraint("sum", vec![0, 1], |v| {
            SimpleScore::new(-v.iter().flatten().map(|v| **v).sum::<i64>())
        })
        .unwrap();
        let mut ln = ListConstraintNetwork::new(2, 3);
        ln.add_constraint(
            "list",
            vec![ListScope::Entity(0), ListScope::Entity(1)],
            |f| {
                let [ListFact::Entity(a), ListFact::Entity(b)] = f else {
                    unreachable!()
                };
                ListAssignment::new(vec![a.to_vec(), b.to_vec()]).objective()
            },
        )
        .unwrap();
        let mut mn = MixedConstraintNetwork::new(2, 2, 3);
        mn.add_constraint(
            "joint",
            vec![
                MixedScope::Basic(0),
                MixedScope::Basic(1),
                MixedScope::ListEntity(0),
                MixedScope::ListEntity(1),
            ],
            |f| {
                let [
                    MixedFact::Basic(a),
                    MixedFact::Basic(b),
                    MixedFact::ListEntity(x),
                    MixedFact::ListEntity(y),
                ] = f
                else {
                    unreachable!()
                };
                MixedAssignment::new(
                    Assignment::new(vec![a.copied(), b.copied()]),
                    ListAssignment::new(vec![x.to_vec(), y.to_vec()]),
                )
                .objective()
            },
        )
        .unwrap();
        for order in ORDERS {
            let mut c = phase(order, PartialScoreBound::Unknown);
            if !completion {
                let ExactSearchStrategy::CompleteBranchAndBound(bounded) = c.strategy else {
                    unreachable!()
                };
                c.strategy = ExactSearchStrategy::BranchAndBound(bounded);
            }
            macro_rules! exercise {
                ($exec:expr, $expected:expr) => {{
                    let exec = $exec;
                    let baseline = exec(SolverExtensions::default());
                    let calls = Cell::new(0u64);
                    let mut unknown = |node: ExactBoundContext<'_, _, SimpleScore>| {
                        calls.set(calls.get() + 1);
                        assert_eq!(
                            node.score.score(),
                            Objective::objective(node.assignment).score()
                        );
                        None
                    };
                    let mut ext = SolverExtensions::default();
                    ext.optimistic_bounds.bind(0, &mut unknown);
                    let unchanged = exec(ext);
                    assert_eq!(baseline.assignment, unchanged.assignment);
                    assert_eq!(baseline.score, unchanged.score);
                    assert_eq!(baseline.work, unchanged.work, "None must not add scoring");
                    assert_eq!(calls.get(), unchanged.work.evaluations);
                    calls.set(0);
                    let mut bound = |node: ExactBoundContext<'_, _, SimpleScore>| {
                        calls.set(calls.get() + 1);
                        // Positive list values incur at least their value as cost,
                        // wherever inserted. All basic domains have minimum zero.
                        Some(SimpleScore::new(
                            node.score.score()
                                - node
                                    .remaining_list_values
                                    .iter()
                                    .map(|i| *i as i64 + 1)
                                    .sum::<i64>(),
                        ))
                    };
                    let mut ext = SolverExtensions::default();
                    ext.optimistic_bounds.bind(0, &mut bound);
                    let pruned = exec(ext);
                    check(&pruned, $expected, completion);
                    assert_eq!(calls.get(), pruned.work.evaluations);
                    assert!(
                        pruned.work.evaluations <= baseline.work.evaluations,
                        "{order:?} completion={completion}: {} vs {}",
                        pruned.work.evaluations,
                        baseline.work.evaluations
                    );
                }};
            }
            exercise!(
                |ext| solve_with_extensions(&b, &bn, plan(c.clone()), ext).unwrap(),
                &eb
            );
            exercise!(
                |ext| solve_list_with_extensions(&l, &ln, plan(c.clone()), ext).unwrap(),
                &el
            );
            exercise!(
                |ext| solve_mixed_with_extensions(&m, &mn, mixed_plan(c.clone()), ext).unwrap(),
                &em
            );
            exercise!(
                |ext| solve_stream_with_extensions(&b, || Ok(basic_graph()), plan(c.clone()), ext)
                    .unwrap(),
                &eb
            );
            exercise!(
                |ext| solve_list_stream_with_extensions(
                    &l,
                    || Ok(list_graph()),
                    plan(c.clone()),
                    ext
                )
                .unwrap(),
                &el
            );
            exercise!(
                |ext| solve_mixed_stream_with_extensions(
                    &m,
                    || Ok(mixed_graph()),
                    mixed_plan(c.clone()),
                    ext
                )
                .unwrap(),
                &em
            );
            exercise!(
                |ext| solve_calculator_with_extensions(
                    &b,
                    &cached::<Vec<usize>>,
                    plan(c.clone()),
                    ext
                )
                .unwrap(),
                &eb
            );
            exercise!(
                |ext| solve_list_calculator_with_extensions(
                    &l,
                    &cached::<Vec<usize>>,
                    plan(c.clone()),
                    ext
                )
                .unwrap(),
                &el
            );
            exercise!(
                |ext| solve_mixed_calculator_with_extensions(
                    &m,
                    &cached::<MixedCalculatorChange>,
                    mixed_plan(c.clone()),
                    ext
                )
                .unwrap(),
                &em
            );
            macro_rules! retained {
                ($d:expr, $plan:expr, $expected:expr) => {
                    exercise!(
                        |ext| {
                            let mut d = $d;
                            let r = d.run_solver_with_extensions($plan, ext).unwrap();
                            assert_eq!(d.assignment(), &r.assignment);
                            d.assert_consistent().unwrap();
                            r
                        },
                        $expected
                    );
                };
            }
            retained!(
                ScoreDirector::new(&b, &bn, b.initial_assignment()).unwrap(),
                plan(c.clone()),
                &eb
            );
            retained!(
                ListScoreDirector::new(&l, &ln, l.initial_assignment()).unwrap(),
                plan(c.clone()),
                &el
            );
            retained!(
                MixedScoreDirector::new(&m, &mn, m.initial_assignment()).unwrap(),
                mixed_plan(c.clone()),
                &em
            );
            retained!(
                {
                    let (g, s) = basic_graph();
                    StreamScoreDirector::new(&b, g, s, b.initial_assignment()).unwrap()
                },
                plan(c.clone()),
                &eb
            );
            retained!(
                {
                    let (g, s) = list_graph();
                    ListStreamScoreDirector::new(&l, g, s, l.initial_assignment()).unwrap()
                },
                plan(c.clone()),
                &el
            );
            retained!(
                {
                    let (g, s) = mixed_graph();
                    MixedStreamScoreDirector::new(&m, g, s, m.initial_assignment()).unwrap()
                },
                mixed_plan(c.clone()),
                &em
            );
            retained!(
                CalculatorScoreDirector::new(
                    &b,
                    b.initial_assignment(),
                    cached::<Vec<usize>>,
                    false
                )
                .unwrap(),
                plan(c.clone()),
                &eb
            );
            retained!(
                ListCalculatorScoreDirector::new(
                    &l,
                    l.initial_assignment(),
                    cached::<Vec<usize>>,
                    false
                )
                .unwrap(),
                plan(c.clone()),
                &el
            );
            retained!(
                MixedCalculatorScoreDirector::new(
                    &m,
                    m.initial_assignment(),
                    cached::<MixedCalculatorChange>,
                    false
                )
                .unwrap(),
                mixed_plan(c.clone()),
                &em
            );
        }
    }
}

#[test]
fn bindings_validate_before_callbacks_and_replace_or_remove_atomically() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1])]).unwrap();
    for (index, strategy) in [
        (1, ExactSearchStrategy::Enumerate),
        (0, ExactSearchStrategy::Enumerate),
        (0, ExactSearchStrategy::Complete),
    ] {
        let mut bound = |_: ExactBoundContext<'_, Assignment<i64>, SimpleScore>| {
            panic!("invalid binding called")
        };
        let mut ext = SolverExtensions::default();
        ext.optimistic_bounds.bind(index, &mut bound);
        let mut lifecycle = |_| panic!("preflight lifecycle");
        ext.lifecycle = Some(&mut lifecycle);
        let mut factory = || -> Result<(StreamGraph<'static, SimpleScore>, PlanningStreamSource<i64>), GraphError> { panic!("preflight factory") };
        let e = solve_stream_with_extensions(
            &p,
            &mut factory,
            plan(ExactPhaseConfig {
                strategy,
                max_evaluations: 0,
                ..Default::default()
            }),
            ext,
        )
        .unwrap_err();
        assert!(matches!(e, SolverError::InvalidOptimisticBoundPhase{phase} if phase==index));
    }
    let mut first = |_: ExactBoundContext<'_, Assignment<i64>, SimpleScore>| panic!("replaced");
    let mut second = |_: ExactBoundContext<'_, Assignment<i64>, SimpleScore>| None;
    let mut bindings = PhaseOptimisticBounds::default();
    assert!(bindings.bind(0, &mut first).is_none());
    assert!(bindings.bind(0, &mut second).is_some());
    assert!(bindings.contains(0));
    assert!(bindings.remove(0).is_some());
    assert!(bindings.remove(0).is_none());
    assert!(bindings.is_empty());
}

#[test]
fn remaining_ids_respect_order_pins_completion_and_processed_optional_omissions() {
    let b = FiniteProblem::new(vec![
        Variable::new(vec![1_i64, 2]).with_initial(1),
        Variable::new(vec![4]).with_initial(4).with_pinned(true),
        Variable::new(vec![8]).allowing_unassigned(),
    ])
    .unwrap();
    let l = ListProblem::new(
        vec![1_i64, 2, 3],
        vec![
            ListEntity::new(vec![1]).with_pinned_prefix(1),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let p = MixedProblem::new(b, l);
    let n = MixedConstraintNetwork::<_, _, SimpleScore>::new(3, 2, 3);
    for completion in [false, true] {
        let mut c = phase(
            NodeExploration::InputOrderDepthFirst,
            PartialScoreBound::Unknown,
        );
        let order = BranchOrder {
            basic_variables: Some(vec![2, 1, 0]),
            list_values: Some(vec![2, 0, 1]),
            ..Default::default()
        };
        let bounded = BoundedPhaseConfig {
            order: Some(order.into()),
            exploration: NodeExploration::InputOrderDepthFirst,
            ..Default::default()
        };
        c.strategy = if completion {
            ExactSearchStrategy::CompleteBranchAndBound(bounded)
        } else {
            ExactSearchStrategy::BranchAndBound(bounded)
        };
        let seen = RefCell::new(Vec::new());
        let mut bound = |node: ExactBoundContext<'_, MixedAssignment<i64, i64>, SimpleScore>| {
            let basic = if completion { vec![2] } else { vec![2, 0] };
            let list = [2, 1];
            assert_eq!(node.remaining_list_values, &list[node.depth.min(2)..]);
            assert_eq!(
                node.remaining_basic_variables,
                &basic[node.depth.saturating_sub(2)..]
            );
            assert_eq!(node.assignment.basic().values()[1], Some(4));
            assert_eq!(node.assignment.lists().lists()[0][0], 1);
            if completion {
                assert_eq!(node.assignment.basic().values()[0], Some(1));
            }
            if node.remaining_basic_variables.is_empty() && node.remaining_list_values.is_empty() {
                seen.borrow_mut().push((
                    node.assignment.basic().values()[2],
                    node.assignment
                        .lists()
                        .lists()
                        .iter()
                        .map(Vec::len)
                        .sum::<usize>(),
                ));
            }
            None
        };
        let mut ext = SolverExtensions::default();
        ext.optimistic_bounds.bind(0, &mut bound);
        let result = solve_mixed_with_extensions(&p, &n, mixed_plan(c), ext).unwrap();
        assert!(result.complete);
        assert!(
            seen.borrow().contains(&(None, 1)),
            "optional omissions are processed leaves"
        );
    }
}

#[test]
fn capacity_bound_beats_unknown_and_preserves_independent_optimum() {
    let weights = [6_i64, 5, 4, 4, 3, 2];
    let profits = [12_i64, 10, 8, 7, 5, 3];
    let p = FiniteProblem::new(vec![Variable::new(vec![1_i64, 0]); 6]).unwrap();
    let calls = Cell::new(0);
    let mut n = ConstraintNetwork::new(6);
    n.add_constraint("capacity profit", (0..6).collect(), |v| {
        calls.set(calls.get() + 1);
        let weight: i64 = v
            .iter()
            .zip(weights)
            .map(|(v, w)| v.copied().unwrap_or(0) * w)
            .sum();
        SimpleScore::new(if weight > 10 {
            -1000
        } else {
            v.iter()
                .zip(profits)
                .map(|(v, w)| v.copied().unwrap_or(0) * w)
                .sum()
        })
    })
    .unwrap();
    // Independent complete bit-mask enumeration, not a search-tree traversal.
    let optimum = (0..64)
        .filter_map(|mask| {
            let weight: i64 = (0..6)
                .filter(|i| mask & (1 << i) != 0)
                .map(|i| weights[i])
                .sum();
            (weight <= 10).then(|| {
                (0..6)
                    .filter(|i| mask & (1 << i) != 0)
                    .map(|i| profits[i])
                    .sum::<i64>()
            })
        })
        .max()
        .unwrap();
    assert_eq!(optimum, 20);
    for order in ORDERS {
        let c = ExactPhaseConfig {
            strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig {
                exploration: order,
                ..Default::default()
            }),
            ..Default::default()
        };
        calls.set(0);
        let baseline = solve(&p, &n, plan(c.clone())).unwrap();
        assert_eq!(calls.get(), baseline.work.evaluations);
        let bound_calls = Cell::new(0);
        let mut bound = |node: ExactBoundContext<'_, Assignment<i64>, SimpleScore>| {
            bound_calls.set(bound_calls.get() + 1);
            let weight: i64 = node
                .assignment
                .values()
                .iter()
                .zip(weights)
                .map(|(v, w)| v.unwrap_or(0) * w)
                .sum();
            if weight > 10 {
                return Some(SimpleScore::new(-1000));
            }
            // Residual capacity dynamic program: no score callback or assignment cloning.
            let capacity = (10 - weight) as usize;
            let mut best = vec![0; capacity + 1];
            for &i in node.remaining_basic_variables {
                for room in (weights[i] as usize..=capacity).rev() {
                    best[room] = best[room].max(best[room - weights[i] as usize] + profits[i]);
                }
            }
            Some(SimpleScore::new(node.score.score() + best[capacity]))
        };
        let mut ext = SolverExtensions::default();
        ext.optimistic_bounds.bind(0, &mut bound);
        calls.set(0);
        let pruned = solve_with_extensions(&p, &n, plan(c), ext).unwrap();
        assert_eq!(calls.get(), pruned.work.evaluations);
        assert_eq!(bound_calls.get(), pruned.work.evaluations);
        assert_eq!(pruned.score, Some(SimpleScore::new(optimum)));
        assert_eq!(pruned.score, baseline.score);
        assert!(pruned.is_optimal());
        assert!(pruned.work.evaluations < baseline.work.evaluations);
        eprintln!(
            "capacity {order:?}: {} -> {} evaluated nodes",
            baseline.work.evaluations, pruned.work.evaluations
        );
    }
}

#[test]
fn callback_budgets_cancellation_time_and_phase_handoffs_are_cooperative() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]); 3]).unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("sum", vec![0, 1, 2], |v| {
        SimpleScore::new(v.iter().flatten().map(|v| **v).sum())
    })
    .unwrap();
    for budget in 0..=15 {
        let calls = Cell::new(0);
        let mut bound = |_: ExactBoundContext<'_, Assignment<i64>, SimpleScore>| {
            calls.set(calls.get() + 1);
            None
        };
        let mut ext = SolverExtensions::default();
        ext.optimistic_bounds.bind(0, &mut bound);
        let mut c = phase(
            NodeExploration::InputOrderDepthFirst,
            PartialScoreBound::Unknown,
        );
        c.max_evaluations = budget;
        let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        let r = d.run_solver_with_extensions(plan(c), ext).unwrap();
        assert_eq!(calls.get(), budget);
        assert_eq!(r.work.evaluations, budget);
        assert_eq!(d.assignment(), &r.assignment);
        d.assert_consistent().unwrap();
    }
    let token = CancellationToken::new();
    let calls = Cell::new(0);
    let mut bound = |_: ExactBoundContext<'_, Assignment<i64>, SimpleScore>| {
        calls.set(calls.get() + 1);
        if calls.get() == 3 {
            token.cancel();
        }
        None
    };
    let mut ext = SolverExtensions::default();
    ext.optimistic_bounds.bind(0, &mut bound);
    let mut c = phase(
        NodeExploration::InputOrderDepthFirst,
        PartialScoreBound::Unknown,
    );
    c.cancellation = Some(&token);
    let r = solve_with_extensions(&p, &n, plan(c), ext).unwrap();
    assert_eq!(r.work.evaluations, 3);
    assert_eq!(calls.get(), 3);
    assert_eq!(r.termination, SolverTermination::Cancelled);

    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    let ticks = Arc::new(AtomicU64::new(0));
    let read = ticks.clone();
    let clock = SolverClock::new(move || Duration::from_millis(read.load(Ordering::Relaxed)));
    let mut bound = |_: ExactBoundContext<'_, Assignment<i64>, SimpleScore>| {
        ticks.store(3, Ordering::Relaxed);
        None
    };
    let mut ext = SolverExtensions {
        clock: Some(&clock),
        ..Default::default()
    };
    ext.optimistic_bounds.bind(0, &mut bound);
    let mut c = phase(
        NodeExploration::InputOrderDepthFirst,
        PartialScoreBound::Unknown,
    );
    c.max_time = Some(Duration::from_millis(2));
    let r = solve_with_extensions(&p, &n, plan(c), ext).unwrap();
    assert_eq!(r.work.evaluations, 1);
    assert_eq!(
        r.phases[0].termination,
        PhaseTermination::Exact(ExactPhaseTermination::TimeLimit)
    );

    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("sum", vec![0], |v| {
        SimpleScore::new(v[0].copied().unwrap_or(0))
    })
    .unwrap();
    let a = Cell::new(0);
    let b = Cell::new(0);
    let mut first = |node: ExactBoundContext<'_, Assignment<i64>, SimpleScore>| {
        a.set(a.get() + 1);
        assert_eq!(node.depth, 0);
        Some(*node.score)
    };
    let mut second = |node: ExactBoundContext<'_, Assignment<i64>, SimpleScore>| {
        b.set(b.get() + 1);
        Some(SimpleScore::new(
            node.score.score() + node.remaining_basic_variables.len() as i64,
        ))
    };
    let mut ext = SolverExtensions::default();
    ext.optimistic_bounds.bind(0, &mut first);
    ext.optimistic_bounds.bind(1, &mut second);
    let c = phase(
        NodeExploration::InputOrderDepthFirst,
        PartialScoreBound::Unknown,
    );
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![
                SolverPhase::Exact(c),
                SolverPhase::Exact(ExactPhaseConfig {
                    strategy: ExactSearchStrategy::BranchAndBound(Default::default()),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        },
        ext,
    )
    .unwrap();
    assert_eq!((a.get(), b.get()), (1, 3));
    assert_eq!(r.score, Some(SimpleScore::new(1)));
    assert_eq!(r.phases.len(), 2);
}

#[test]
fn overrides_static_bounds_and_preserves_failure_and_leaf_diagnostics() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1_i64, 2])]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("increasing", vec![0], |v| {
        SimpleScore::new(v[0].copied().unwrap_or(0))
    })
    .unwrap();
    let c = phase(
        NodeExploration::InputOrderDepthFirst,
        PartialScoreBound::NonIncreasing,
    );
    assert!(matches!(
        solve(&p, &n, plan(c.clone())),
        Err(SolverError::InvalidOptimisticBound { .. })
    ));
    let mut unknown = |_: ExactBoundContext<'_, Assignment<i64>, SimpleScore>| None;
    let mut ext = SolverExtensions::default();
    ext.optimistic_bounds.bind(0, &mut unknown);
    assert_eq!(
        solve_with_extensions(&p, &n, plan(c.clone()), ext)
            .unwrap()
            .score,
        Some(SimpleScore::new(2))
    );
    let invalid_trend = phase(
        NodeExploration::DepthFirst,
        PartialScoreBound::Trend(InitializingScoreTrend::new(
            vec![InitializingScoreTrendLevel::Any; 2],
        )),
    );
    assert!(matches!(
        solve(&p, &n, plan(invalid_trend.clone())),
        Err(SolverError::InvalidScoreTrend { .. })
    ));
    let mut valid = |node: ExactBoundContext<'_, Assignment<i64>, SimpleScore>| {
        Some(SimpleScore::new(
            node.score.score() + 2 * node.remaining_basic_variables.len() as i64,
        ))
    };
    let mut ext = SolverExtensions::default();
    ext.optimistic_bounds.bind(0, &mut valid);
    assert_eq!(
        solve_with_extensions(&p, &n, plan(invalid_trend), ext)
            .unwrap()
            .score,
        Some(SimpleScore::new(2))
    );
    let mut bad =
        |_: ExactBoundContext<'_, Assignment<i64>, SimpleScore>| Some(SimpleScore::new(-10));
    let mut ext = SolverExtensions::default();
    ext.optimistic_bounds.bind(0, &mut bad);
    assert!(matches!(
        solve_with_extensions(&p, &n, plan(c.clone()), ext),
        Err(SolverError::InvalidOptimisticBound { phase: 0, .. })
    ));
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let mut panic_bound =
        |_: ExactBoundContext<'_, Assignment<i64>, SimpleScore>| -> Option<SimpleScore> {
            panic!("bound panic")
        };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut ext = SolverExtensions::default();
        ext.optimistic_bounds.bind(0, &mut panic_bound);
        d.run_solver_with_extensions(plan(c), ext)
    }));
    assert!(result.is_err());
    d.assert_consistent().unwrap();
    let mut invalid = ConstraintNetwork::new(1);
    invalid
        .add_constraint("invalid", vec![0], |_| {
            SimpleScore::new(0).with_structural(-1)
        })
        .unwrap();
    let mut never =
        |_: ExactBoundContext<'_, Assignment<i64>, SimpleScore>| panic!("bound on failed score");
    let mut ext = SolverExtensions::default();
    ext.optimistic_bounds.bind(0, &mut never);
    assert!(matches!(
        solve_with_extensions(
            &p,
            &invalid,
            plan(phase(
                NodeExploration::DepthFirst,
                PartialScoreBound::Unknown
            )),
            ext
        ),
        Err(SolverError::Phase { .. })
    ));
}
