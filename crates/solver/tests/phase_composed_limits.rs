//! Original phase-local expression and continuation checks across all 18 routes.
use std::marker::PhantomData;
use timetuck::*;
fn plan<'a>(_condition: &'a SearchTermination<'a>) -> SolverSequence<'a> {
    SolverSequence {
        phases: vec![
            SolverPhase::RuinRecreate(RuinRecreateConfig {
                minimum_ruined: 99,
                maximum_ruined: 99,
                max_attempts: 4,
                ..Default::default()
            });
            2
        ],
        limits: SolverLimits::default(),
    }
}
fn mixed_plan<'a>(condition: &'a SearchTermination<'a>) -> MixedSolverSequence<'a> {
    let p = plan(condition);
    let SolverPhase::RuinRecreate(search) = p.phases[0].clone() else {
        unreachable!()
    };
    MixedSolverSequence {
        phases: vec![
            MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                search,
                ..Default::default()
            });
            2
        ],
        limits: p.limits,
    }
}
fn check<A: Objective + std::fmt::Debug + PartialEq>(
    r: SolverResult<A, SimpleScore>,
    expected: &A,
    n: u64,
    metrics: &SolverMetrics<SimpleScore>,
) {
    assert_eq!(&r.assignment, expected);
    assert_eq!(r.score, Some(expected.objective()));
    assert!(r.complete);
    assert_eq!(r.termination, SolverTermination::PlanCompleted);
    assert_eq!(r.phases.len(), 2);
    assert_eq!(
        r.phases[1].termination,
        PhaseTermination::RuinRecreate(RuinRecreateTermination::Condition)
    );
    assert_eq!(
        r.phases[0].termination,
        PhaseTermination::RuinRecreate(RuinRecreateTermination::Condition)
    );
    assert_eq!(r.work.evaluations, 2 * (n + 2)); // Solve initialization, partial repair initialization, n trials.
    assert_eq!(r.work.move_evaluations, 2 * n);
    assert_eq!(r.work.completed_steps, 0);
    let counts = metrics.snapshot().latest.unwrap().move_evaluations;
    assert_eq!(counts.total(), 2 * n);
    assert_eq!(counts.get(MoveEvaluationKind::Reconstruction), 2 * n);
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
#[test]
fn all_routes_restart_phase_counters_and_continue_after_nested_repair_stops() {
    let one = SearchTermination::Condition(SearchLimit::MoveEvaluations(1));
    routes(&one, 1);
    let both = [
        SearchTermination::Condition(SearchLimit::MoveEvaluationsOf {
            kind: MoveEvaluationKind::Reconstruction,
            limit: 2,
        }),
        SearchTermination::Condition(SearchLimit::ScoreEvaluations(4)),
    ];
    let two = SearchTermination::All(&both);
    routes(&two, 2);
    let three = [
        SearchTermination::Condition(SearchLimit::MoveEvaluations(3)),
        SearchTermination::Condition(SearchLimit::Steps(1000)),
    ];
    let three = SearchTermination::Any(&three);
    routes(&three, 3);
}
fn routes(c: &SearchTermination<'_>, expected_count: u64) {
    let b = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(0),
    ])
    .unwrap();
    let l = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![3, 2, 1]), ListEntity::new(vec![])],
    )
    .unwrap();
    let m = MixedProblem::new(b.clone(), l.clone());
    let eb = Assignment::new(vec![Some(0), Some(0)]);
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
    let metrics = SolverMetrics::new(None);
    let bindings = [
        PhaseTerminationBinding::search(0, c),
        PhaseTerminationBinding::search(1, c),
    ];
    macro_rules! extensions {
        () => {
            SolverExtensions {
                metrics: Some(&metrics),
                phase_terminations: &bindings,
                ..Default::default()
            }
        };
    }
    check(
        solve_with_extensions(&b, &bn, plan(c), extensions!()).unwrap(),
        &eb,
        expected_count,
        &metrics,
    );
    check(
        solve_list_with_extensions(&l, &ln, plan(c), extensions!()).unwrap(),
        &el,
        expected_count,
        &metrics,
    );
    check(
        solve_mixed_with_extensions(&m, &mn, mixed_plan(c), extensions!()).unwrap(),
        &em,
        expected_count,
        &metrics,
    );
    check(
        solve_stream_with_extensions(&b, || Ok(basic_graph()), plan(c), extensions!()).unwrap(),
        &eb,
        expected_count,
        &metrics,
    );
    check(
        solve_list_stream_with_extensions(&l, || Ok(list_graph()), plan(c), extensions!()).unwrap(),
        &el,
        expected_count,
        &metrics,
    );
    check(
        solve_mixed_stream_with_extensions(&m, || Ok(mixed_graph()), mixed_plan(c), extensions!())
            .unwrap(),
        &em,
        expected_count,
        &metrics,
    );
    check(
        solve_calculator_with_extensions(&b, &cached::<Vec<usize>>, plan(c), extensions!())
            .unwrap(),
        &eb,
        expected_count,
        &metrics,
    );
    check(
        solve_list_calculator_with_extensions(&l, &cached::<Vec<usize>>, plan(c), extensions!())
            .unwrap(),
        &el,
        expected_count,
        &metrics,
    );
    check(
        solve_mixed_calculator_with_extensions(
            &m,
            &cached::<MixedCalculatorChange>,
            mixed_plan(c),
            extensions!(),
        )
        .unwrap(),
        &em,
        expected_count,
        &metrics,
    );
    macro_rules! retained {
        ($d:expr,$plan:expr,$expected:expr) => {{
            let mut d = $d;
            let r = d.run_solver_with_extensions($plan, extensions!()).unwrap();
            assert_eq!(d.assignment(), &r.assignment);
            d.assert_consistent().unwrap();
            check(r, $expected, expected_count, &metrics);
        }};
    }
    retained!(
        ScoreDirector::new(&b, &bn, b.initial_assignment()).unwrap(),
        plan(c),
        &eb
    );
    retained!(
        ListScoreDirector::new(&l, &ln, l.initial_assignment()).unwrap(),
        plan(c),
        &el
    );
    retained!(
        MixedScoreDirector::new(&m, &mn, m.initial_assignment()).unwrap(),
        mixed_plan(c),
        &em
    );
    let (g, s) = basic_graph();
    retained!(
        StreamScoreDirector::new(&b, g, s, b.initial_assignment()).unwrap(),
        plan(c),
        &eb
    );
    let (g, s) = list_graph();
    retained!(
        ListStreamScoreDirector::new(&l, g, s, l.initial_assignment()).unwrap(),
        plan(c),
        &el
    );
    let (g, s) = mixed_graph();
    retained!(
        MixedStreamScoreDirector::new(&m, g, s, m.initial_assignment()).unwrap(),
        mixed_plan(c),
        &em
    );
    retained!(
        CalculatorScoreDirector::new(&b, b.initial_assignment(), cached::<Vec<usize>>, false)
            .unwrap(),
        plan(c),
        &eb
    );
    retained!(
        ListCalculatorScoreDirector::new(&l, l.initial_assignment(), cached::<Vec<usize>>, false)
            .unwrap(),
        plan(c),
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
