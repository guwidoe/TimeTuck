//! Original bounded completion contracts; no upstream method credit.
use std::{cell::Cell, collections::BTreeSet, marker::PhantomData, time::Duration};
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
fn check<A: Objective + std::fmt::Debug + PartialEq, const COMPLETE: bool>(
    r: SolverResult<A, SimpleScore>,
    expected: &A,
) {
    assert_eq!(&r.assignment, expected);
    assert_eq!(r.score, Some(expected.objective()));
    assert!(r.complete);
    assert_eq!(r.is_optimal(), !COMPLETE);
    assert_eq!(
        r.termination,
        if COMPLETE {
            SolverTermination::PlanCompleted
        } else {
            SolverTermination::Optimal
        }
    );
    assert_eq!(
        r.phases[0].termination,
        PhaseTermination::Exact(if COMPLETE {
            ExactPhaseTermination::CompletionExhausted
        } else {
            ExactPhaseTermination::Exhausted
        })
    );
    assert_eq!(r.work.evaluations, r.work.move_evaluations);
    assert!(r.work.completed_steps > 0);
}
#[test]
fn all_eighteen_routes_preserve_anchors_and_restore_best_under_every_order() {
    all_eighteen_routes::<true>();
}
#[test]
fn all_eighteen_unrestricted_routes_restore_best_under_every_order() {
    all_eighteen_routes::<false>();
}
fn all_eighteen_routes<const COMPLETE: bool>() {
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
    let eb = Assignment::new(vec![Some(if COMPLETE { 2 } else { 0 }), Some(0)]);
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
        for bound in [PartialScoreBound::Unknown, PartialScoreBound::NonIncreasing] {
            let mut c = phase(order, bound);
            if !COMPLETE {
                let ExactSearchStrategy::CompleteBranchAndBound(bounded) = c.strategy else {
                    unreachable!()
                };
                c.strategy = ExactSearchStrategy::BranchAndBound(bounded);
            }
            check::<_, COMPLETE>(solve(&b, &bn, plan(c.clone())).unwrap(), &eb);
            check::<_, COMPLETE>(solve_list(&l, &ln, plan(c.clone())).unwrap(), &el);
            check::<_, COMPLETE>(solve_mixed(&m, &mn, mixed_plan(c.clone())).unwrap(), &em);
            check::<_, COMPLETE>(
                solve_stream(&b, || Ok(basic_graph()), plan(c.clone())).unwrap(),
                &eb,
            );
            check::<_, COMPLETE>(
                solve_list_stream(&l, || Ok(list_graph()), plan(c.clone())).unwrap(),
                &el,
            );
            check::<_, COMPLETE>(
                solve_mixed_stream(&m, || Ok(mixed_graph()), mixed_plan(c.clone())).unwrap(),
                &em,
            );
            check::<_, COMPLETE>(
                solve_calculator(&b, &cached::<Vec<usize>>, plan(c.clone())).unwrap(),
                &eb,
            );
            check::<_, COMPLETE>(
                solve_list_calculator(&l, &cached::<Vec<usize>>, plan(c.clone())).unwrap(),
                &el,
            );
            check::<_, COMPLETE>(
                solve_mixed_calculator(&m, &cached::<MixedCalculatorChange>, mixed_plan(c.clone()))
                    .unwrap(),
                &em,
            );
            macro_rules! retained {
                ($d:expr,$plan:expr,$expected:expr) => {{
                    let mut d = $d;
                    let r = d.run_solver($plan).unwrap();
                    assert_eq!(d.assignment(), &r.assignment);
                    d.assert_consistent().unwrap();
                    check::<_, COMPLETE>(r, $expected);
                }};
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
            let (g, s) = basic_graph();
            retained!(
                StreamScoreDirector::new(&b, g, s, b.initial_assignment()).unwrap(),
                plan(c.clone()),
                &eb
            );
            let (g, s) = list_graph();
            retained!(
                ListStreamScoreDirector::new(&l, g, s, l.initial_assignment()).unwrap(),
                plan(c.clone()),
                &el
            );
            let (g, s) = mixed_graph();
            retained!(
                MixedStreamScoreDirector::new(&m, g, s, m.initial_assignment()).unwrap(),
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
                mixed_plan(c),
                &em
            );
        }
    }
}
#[test]
fn list_leaves_match_independent_enumeration_with_optional_values_pins_and_ranges() {
    for optional in [false, true] {
        for order in ORDERS {
            let p = ListProblem::new(
                vec![1, 2, 3, 4, 5],
                vec![
                    ListEntity::new(vec![5]).with_pinned(true),
                    ListEntity::new(vec![1, 3])
                        .with_pinned_prefix(1)
                        .with_value_range(vec![1, 2, 3]),
                    ListEntity::new(vec![]).with_value_range(vec![2, 4]),
                ],
            )
            .unwrap();
            let p = if optional { p.allowing_unassigned() } else { p };
            let mut expected = BTreeSet::new();
            visit_list_completions(&p, &p.initial_assignment(), |a| {
                expected.insert(a.lists().to_vec());
                true
            })
            .unwrap();
            let seen = std::cell::RefCell::new(BTreeSet::new());
            let callbacks = Cell::new(0);
            let mut n = ListConstraintNetwork::new(3, 5);
            n.add_constraint(
                "trace",
                vec![
                    ListScope::Entity(0),
                    ListScope::Entity(1),
                    ListScope::Entity(2),
                ],
                |f| {
                    callbacks.set(callbacks.get() + 1);
                    let rows = f
                        .iter()
                        .map(|v| {
                            let ListFact::Entity(v) = v else {
                                unreachable!()
                            };
                            v.to_vec()
                        })
                        .collect::<Vec<_>>();
                    // Optional intermediate states are also valid omitted-value completions;
                    // all scored states must equal the independent completion set.
                    if optional || rows.iter().map(Vec::len).sum::<usize>() == 5 {
                        seen.borrow_mut().insert(rows);
                    }
                    SimpleScore::ZERO
                },
            )
            .unwrap();
            let r = solve_list(&p, &n, plan(phase(order, PartialScoreBound::Unknown))).unwrap();
            assert!(expected.contains(r.assignment.lists()));
            assert_eq!(*seen.borrow(), expected);
            assert_eq!(r.work.constraint_evaluations, callbacks.get());
            assert_eq!(
                r.phases[0].termination,
                PhaseTermination::Exact(ExactPhaseTermination::CompletionExhausted)
            );
            assert!(!r.is_optimal());
        }
    }
}
#[test]
fn root_partial_and_leaf_evaluations_share_exact_phase_and_solve_budgets() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(1),
        Variable::new(vec![0, 1]),
        Variable::new(vec![0, 1]),
    ])
    .unwrap();
    for order in ORDERS {
        for cap in 0..=8 {
            for scope in 0..4 {
                let calls = Cell::new(0);
                let mut n = ConstraintNetwork::new(3);
                n.add_constraint("sum", vec![0, 1, 2], |v| {
                    calls.set(calls.get() + 1);
                    SimpleScore::new(-v.iter().flatten().map(|v| **v).sum::<i64>())
                })
                .unwrap();
                let mut c = phase(order, PartialScoreBound::Unknown);
                if scope == 0 {
                    c.max_evaluations = cap;
                }
                if scope == 1 {
                    c.max_move_evaluations = Some(cap);
                }
                let mut c = plan(c);
                if scope == 2 {
                    c.limits.max_evaluations = cap;
                }
                if scope == 3 {
                    c.limits.max_move_evaluations = Some(cap);
                }
                let r = solve(&p, &n, c).unwrap();
                assert_eq!(r.work.evaluations, cap.min(7));
                assert_eq!(r.work.move_evaluations, cap.min(7));
                assert_eq!(calls.get(), u128::from(cap.min(7)));
                assert!(!r.is_optimal());
                assert_eq!(r.assignment.values()[0], Some(1));
                if cap >= 7 {
                    assert_eq!(r.score, Some(SimpleScore::new(-1)));
                    assert_eq!(r.termination, SolverTermination::PlanCompleted);
                }
            }
        }
    }
}
#[test]
fn admissible_bounds_prune_completion_space_and_exhaustion_allows_later_improvement() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(1),
        Variable::new(vec![0, 1]),
        Variable::new(vec![0, 1]),
        Variable::new(vec![0, 1]),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(4);
    n.add_constraint("sum", vec![0, 1, 2, 3], |v| {
        SimpleScore::new(-v.iter().flatten().map(|v| **v).sum::<i64>())
    })
    .unwrap();
    for order in ORDERS {
        let unknown = solve(&p, &n, plan(phase(order, PartialScoreBound::Unknown))).unwrap();
        let bounded = solve(&p, &n, plan(phase(order, PartialScoreBound::NonIncreasing))).unwrap();
        assert_eq!(bounded.score, unknown.score);
        assert!(bounded.work.evaluations < unknown.work.evaluations);
        let mut c = plan(phase(order, PartialScoreBound::NonIncreasing));
        c.phases
            .push(SolverPhase::Exact(ExactPhaseConfig::default()));
        let r = solve(&p, &n, c).unwrap();
        assert_eq!(r.phases.len(), 2);
        assert!(r.is_optimal());
        assert_eq!(r.score, Some(SimpleScore::ZERO));
    }
}
#[test]
fn cancellation_time_and_frontier_interruptions_preserve_retained_state() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(1),
        Variable::new(vec![0, 1]),
        Variable::new(vec![0, 1]),
    ])
    .unwrap();
    for order in ORDERS {
        for stop in 0..3 {
            let token = CancellationToken::new();
            let calls = Cell::new(0);
            let mut n = ConstraintNetwork::new(3);
            n.add_constraint("sum", vec![0, 1, 2], |v| {
                calls.set(calls.get() + 1);
                if stop == 0 && calls.get() == 4 {
                    token.cancel();
                }
                SimpleScore::new(-v.iter().flatten().map(|v| **v).sum::<i64>())
            })
            .unwrap();
            let mut c = phase(order, PartialScoreBound::Unknown);
            c.cancellation = Some(&token);
            if stop == 1 {
                c.max_time = Some(Duration::ZERO);
            }
            if stop == 2 {
                let ExactSearchStrategy::CompleteBranchAndBound(b) = &mut c.strategy else {
                    unreachable!()
                };
                b.max_frontier = 1;
            }
            let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
            calls.set(0);
            let r = d.run_solver(plan(c)).unwrap();
            assert_eq!(d.assignment(), &r.assignment);
            assert_eq!(r.assignment.values()[0], Some(1));
            assert!(!r.is_optimal());
            if let Some(s) = r.score {
                assert_eq!(d.score(), s);
                assert_eq!(r.assignment.objective(), s);
            }
            let reason = match stop {
                0 => ExactPhaseTermination::Cancelled,
                1 => ExactPhaseTermination::TimeLimit,
                _ => ExactPhaseTermination::FrontierLimit,
            };
            assert_eq!(r.phases[0].termination, PhaseTermination::Exact(reason));
        }
    }
}
#[test]
fn impossible_required_domain_is_not_scored_in_basic_or_mixed_completion() {
    let b = FiniteProblem::<i64>::new(vec![Variable::new(vec![])]).unwrap();
    let l = ListProblem::new(vec![1i64], vec![ListEntity::new(vec![])]).unwrap();
    let m = MixedProblem::new(b.clone(), l);
    let mut bn = ConstraintNetwork::<_, SimpleScore>::new(1);
    bn.add_constraint("unreachable", vec![0], |_| panic!("impossible root scored"))
        .unwrap();
    let mut mn = MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 1, 1);
    mn.add_constraint("unreachable", vec![MixedScope::Basic(0)], |_| {
        panic!("impossible mixed root scored")
    })
    .unwrap();
    let r = solve(
        &b,
        &bn,
        plan(phase(
            NodeExploration::DepthFirst,
            PartialScoreBound::Unknown,
        )),
    )
    .unwrap();
    assert_eq!(r.work.evaluations, 0);
    assert!(!r.complete);
    assert!(!r.is_optimal());
    let r = solve_mixed(
        &m,
        &mn,
        mixed_plan(phase(
            NodeExploration::DepthFirst,
            PartialScoreBound::Unknown,
        )),
    )
    .unwrap();
    assert_eq!(r.work.evaluations, 0);
    assert!(!r.complete);
    assert!(!r.is_optimal());
}
#[test]
fn completion_uses_actual_handoff_payloads_and_rebases_retained_cache() {
    #[derive(Clone, Debug)]
    struct V {
        id: u8,
        payload: i64,
    }
    impl PartialEq for V {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for V {}
    let v = |id, payload| V { id, payload };
    let p = FiniteProblem::new(vec![
        Variable::new(vec![v(0, 1), v(1, 2)]).with_initial(v(0, 1)),
        Variable::new(vec![v(0, 3), v(1, 4)]),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::<V, SimpleScore>::new(2);
    for i in 0..2 {
        n.add_constraint(format!("v{i}"), vec![i], |v| {
            SimpleScore::new(v[0].map_or(0, |v| v.payload))
        })
        .unwrap();
    }
    let entry = Assignment::new(vec![Some(v(1, 100)), None]);
    for order in ORDERS {
        let mut d = ScoreDirector::new(&p, &n, entry.clone()).unwrap();
        let c = phase(order, PartialScoreBound::Unknown);
        let mut sequence = plan(c.clone());
        sequence.phases.push(SolverPhase::Exact(c));
        let r = d.run_solver(sequence).unwrap();
        assert_eq!(r.score, Some(SimpleScore::new(104)));
        assert_eq!(r.assignment.values()[0].as_ref().unwrap().id, 1);
        assert_eq!(r.assignment.values()[0].as_ref().unwrap().payload, 100);
        assert_eq!(r.phases[0].work.evaluations, 3);
        assert_eq!(r.phases[1].work.evaluations, 1);
        assert_eq!(d.score(), SimpleScore::new(104));
        d.assert_consistent().unwrap();
    }
    // Existing list values retain their representatives as later canonical values
    // are inserted before/after them, and then the retained best is restored.
    let p = ListProblem::new(
        vec![v(0, 1), v(1, 2), v(2, 3)],
        vec![ListEntity::new(vec![v(0, 1)])],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::<V, SimpleScore>::new(1, 3);
    n.add_constraint("weighted", vec![ListScope::Entity(0)], |f| {
        let ListFact::Entity(v) = f[0] else {
            unreachable!()
        };
        SimpleScore::new(
            v.iter()
                .enumerate()
                .map(|(i, v)| (i + 1) as i64 * v.payload)
                .sum(),
        )
    })
    .unwrap();
    for order in ORDERS {
        let entry = ListAssignment::new(vec![vec![v(2, 90), v(0, 100)]]);
        let mut d = ListScoreDirector::new(&p, &n, entry).unwrap();
        let r = d
            .run_solver(plan(phase(order, PartialScoreBound::Unknown)))
            .unwrap();
        let actual = r.assignment.lists()[0]
            .iter()
            .map(|v| (v.id, v.payload))
            .collect::<Vec<_>>();
        assert_eq!(actual, [(1, 2), (2, 90), (0, 100)]);
        assert_eq!(r.score, Some(SimpleScore::new(482)));
        assert_eq!(d.score(), SimpleScore::new(482));
        d.assert_consistent().unwrap();
    }
}
#[test]
fn dependencies_restrict_insertions_without_reordering_existing_values() {
    let p = ListProblem::new(
        vec![1i64, 2, 3, 4],
        vec![
            ListEntity::new(vec![4]).with_pinned(true),
            ListEntity::new(vec![1, 3])
                .with_pinned_prefix(1)
                .with_value_range(vec![1, 2, 3]),
        ],
    )
    .unwrap()
    .with_dependencies(vec![vec![], vec![0], vec![1], vec![]])
    .unwrap();
    let n = ListConstraintNetwork::<_, SimpleScore>::new(2, 4);
    for order in ORDERS {
        let r = solve_list(&p, &n, plan(phase(order, PartialScoreBound::Unknown))).unwrap();
        assert_eq!(r.assignment.lists(), [vec![4], vec![1, 2, 3]]);
        assert_eq!(r.work.evaluations, 2);
        assert_eq!(
            r.phases[0].termination,
            PhaseTermination::Exact(ExactPhaseTermination::CompletionExhausted)
        );
    }
}
#[test]
fn custom_partial_handoff_and_optional_pinned_null_are_completion_anchors() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0i64, 2]).with_initial(0),
        Variable::new(vec![0, 1]),
        Variable::new(vec![9])
            .with_pinned(true)
            .allowing_unassigned(),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("sum", vec![0, 1, 2], |v| {
        SimpleScore::new(v.iter().flatten().map(|v| **v).sum())
    })
    .unwrap();
    for order in ORDERS {
        let mut c = plan(phase(order, PartialScoreBound::Unknown));
        c.phases.insert(0, SolverPhase::Custom(Default::default()));
        let mut command = |_,
                           ctx: &mut CustomPhaseContext<
            '_,
            Assignment<i64>,
            Move<i64>,
            SimpleScore,
            ScoringError,
        >| {
            ctx.execute(&Move::Change {
                variable: 0,
                value: Some(2),
            })?;
            Ok(())
        };
        let r = solve_with_commands(&p, &n, c, &mut command).unwrap();
        assert_eq!(r.assignment.values(), [Some(2), Some(1), None]);
        assert_eq!(r.score, Some(SimpleScore::new(3)));
        assert_eq!(r.phases.len(), 2);
        assert_eq!(r.phases[1].work.evaluations, 3);
        assert!(!r.is_optimal());
    }
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0i64, 2])
            .with_initial(2)
            .allowing_unassigned(),
        Variable::new(vec![0, 1]).allowing_unassigned(),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("sum", vec![0, 1], |v| {
        SimpleScore::new(v.iter().flatten().map(|v| **v).sum())
    })
    .unwrap();
    let r = solve(
        &p,
        &n,
        plan(phase(
            NodeExploration::DepthFirst,
            PartialScoreBound::Unknown,
        )),
    )
    .unwrap();
    assert_eq!(r.assignment.values(), [Some(2), Some(1)]);
    assert_eq!(r.work.evaluations, 4);
    assert_eq!(r.score, Some(SimpleScore::new(3)));
    assert!(!r.is_optimal());
}
