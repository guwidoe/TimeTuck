//! Original ordered bounded-phase route checks; no upstream method credit.
use std::marker::PhantomData;
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
fn plan(mut c: ExactPhaseConfig<'_>) -> SolverSequence<'_> {
    set_order(
        &mut c,
        BranchOrder {
            basic_variables: Some(vec![1, 0]),
            basic_values: Some(vec![vec![2, 1, 0], vec![1, 0]]),
            ..Default::default()
        },
    );
    SolverSequence {
        phases: vec![SolverPhase::Exact(c)],
        ..Default::default()
    }
}
fn mixed_plan(mut c: ExactPhaseConfig<'_>) -> MixedSolverSequence<'_> {
    set_order(
        &mut c,
        BranchOrder {
            basic_variables: Some(vec![1, 0]),
            basic_values: Some(vec![vec![2, 1, 0], vec![1, 0]]),
            list_values: Some(vec![2, 1, 0]),
            list_entities: Some(vec![1, 0]),
        },
    );
    MixedSolverSequence {
        phases: vec![MixedSolverPhase::Exact(c)],
        ..Default::default()
    }
}
fn set_order(c: &mut ExactPhaseConfig<'_>, order: BranchOrder) {
    let (ExactSearchStrategy::BranchAndBound(b) | ExactSearchStrategy::CompleteBranchAndBound(b)) =
        &mut c.strategy
    else {
        unreachable!()
    };
    b.order = Some(order.into());
}
fn list_plan(mut c: ExactPhaseConfig<'_>) -> SolverSequence<'_> {
    set_order(
        &mut c,
        BranchOrder {
            list_values: Some(vec![2, 1, 0]),
            list_entities: Some(vec![1, 0]),
            ..Default::default()
        },
    );
    SolverSequence {
        phases: vec![SolverPhase::Exact(c)],
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
    assert_eq!(r.work.completed_steps, 0);
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
            check::<_, COMPLETE>(solve_list(&l, &ln, list_plan(c.clone())).unwrap(), &el);
            check::<_, COMPLETE>(solve_mixed(&m, &mn, mixed_plan(c.clone())).unwrap(), &em);
            check::<_, COMPLETE>(
                solve_stream(&b, || Ok(basic_graph()), plan(c.clone())).unwrap(),
                &eb,
            );
            check::<_, COMPLETE>(
                solve_list_stream(&l, || Ok(list_graph()), list_plan(c.clone())).unwrap(),
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
                solve_list_calculator(&l, &cached::<Vec<usize>>, list_plan(c.clone())).unwrap(),
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
                list_plan(c.clone()),
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
                list_plan(c.clone()),
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
                list_plan(c.clone()),
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
