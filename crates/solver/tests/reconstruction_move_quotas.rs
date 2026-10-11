//! Original live reconstruction quota contracts across all 18 model/backend routes.
use std::marker::PhantomData;
use timetuck::*;
fn plan<'a>(condition: &'a SearchTermination<'a>) -> SolverSequence<'a> {
    SolverSequence {
        phases: vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
            minimum_ruined: 99,
            maximum_ruined: 99,
            max_attempts: 4,
            ..Default::default()
        })],
        limits: SolverLimits {
            termination: Some(condition),
            ..Default::default()
        },
    }
}
fn mixed_plan<'a>(condition: &'a SearchTermination<'a>) -> MixedSolverSequence<'a> {
    let p = plan(condition);
    let SolverPhase::RuinRecreate(search) = p.phases[0].clone() else {
        unreachable!()
    };
    MixedSolverSequence {
        phases: vec![MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
            search,
            ..Default::default()
        })],
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
    assert_eq!(r.termination, SolverTermination::Condition);
    assert_eq!(
        r.phases[0].termination,
        PhaseTermination::RuinRecreate(RuinRecreateTermination::Condition)
    );
    assert_eq!(r.work.evaluations, n + 2); // Solve initialization, partial repair initialization, n trials.
    assert_eq!(r.work.move_evaluations, n);
    assert_eq!(r.work.completed_steps, 0);
    let counts = metrics.snapshot().latest.unwrap().move_evaluations;
    assert_eq!(counts.total(), n);
    assert_eq!(counts.get(MoveEvaluationKind::Reconstruction), n);
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
fn all_routes_stop_inside_reconstruction_for_nested_move_expressions() {
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
    macro_rules! extensions {
        () => {
            SolverExtensions {
                metrics: Some(&metrics),
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

#[test]
fn conjunction_waits_for_real_outer_step_and_preserves_family_handoffs() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(v[0].copied().unwrap_or(0))
    })
    .unwrap();
    let leaves = [
        SearchTermination::Condition(SearchLimit::MoveEvaluations(1)),
        SearchTermination::Condition(SearchLimit::Steps(1)),
    ];
    let condition = SearchTermination::All(&leaves);
    let metrics = SolverMetrics::new(None);
    let r = solve_with_extensions(
        &p,
        &n,
        plan(&condition),
        SolverExtensions {
            metrics: Some(&metrics),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.termination, SolverTermination::Condition);
    assert_eq!(r.work.move_evaluations, 2); // First leaf alone must not interrupt reconstruction.
    assert_eq!(r.work.completed_steps, 1);
    assert_eq!(r.assignment.values(), &[Some(1)]);
    assert_eq!(
        metrics
            .snapshot()
            .latest
            .unwrap()
            .move_evaluations
            .get(MoveEvaluationKind::Reconstruction),
        2
    );

    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1])]).unwrap();
    let leaves = [
        SearchTermination::Condition(SearchLimit::MoveEvaluationsOf {
            kind: MoveEvaluationKind::Reconstruction,
            limit: 1,
        }),
        SearchTermination::Condition(SearchLimit::MoveEvaluationsOf {
            kind: MoveEvaluationKind::Change,
            limit: 3,
        }),
    ];
    let condition = SearchTermination::All(&leaves);
    let mut config = plan(&condition);
    let SolverPhase::RuinRecreate(ref mut repair) = config.phases[0] else {
        unreachable!()
    };
    repair.max_attempts = 1;
    config
        .phases
        .insert(0, SolverPhase::Construction(Default::default()));
    config
        .phases
        .push(SolverPhase::LocalSearch(Default::default()));
    let r = solve_with_extensions(
        &p,
        &n,
        config,
        SolverExtensions {
            metrics: Some(&metrics),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.phases.len(), 3);
    assert_eq!(r.termination, SolverTermination::Condition);
    let counts = metrics.snapshot().latest.unwrap().move_evaluations;
    assert_eq!(counts.get(MoveEvaluationKind::Reconstruction), 2);
    assert_eq!(counts.get(MoveEvaluationKind::Change), 3); // Two construction candidates, one local trial.
    assert_eq!(counts.total(), r.work.move_evaluations);
}

#[test]
fn panicked_reconstruction_does_not_publish_pending_counts_or_leak_to_next_solve() {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2, 3]).with_initial(0)]).unwrap();
    let calls = Arc::new(AtomicU64::new(0));
    let metrics = SolverMetrics::new(None);
    let condition = SearchTermination::Condition(SearchLimit::MoveEvaluationsOf {
        kind: MoveEvaluationKind::Reconstruction,
        limit: 2,
    });
    let factory = || {
        let calls = calls.clone();
        let mut g = StreamGraphBuilder::new();
        let source = g.source::<usize, (usize, Option<i64>)>();
        g.score("fallible", source.port(), move |(_, value)| {
            let call = calls.fetch_add(1, Ordering::SeqCst) + 1;
            assert_ne!(call, 4, "second reconstruction candidate fails");
            SimpleScore::new(-value.unwrap_or(0))
        })
        .unwrap();
        Ok((g.build(), source))
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        solve_stream_with_extensions(
            &p,
            factory,
            plan(&condition),
            SolverExtensions {
                metrics: Some(&metrics),
                ..Default::default()
            },
        )
    }));
    assert!(result.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    assert_eq!(
        metrics.snapshot().latest.unwrap().move_evaluations.total(),
        0
    );
    let result = solve_stream_with_extensions(
        &p,
        factory,
        plan(&condition),
        SolverExtensions {
            metrics: Some(&metrics),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.termination, SolverTermination::Condition);
    assert_eq!(result.work.move_evaluations, 2);
    assert_eq!(
        metrics.snapshot().latest.unwrap().move_evaluations.total(),
        2
    );
}

#[test]
fn scored_conjunction_and_cancellation_observe_only_real_candidate_work() {
    use std::cell::Cell;
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2, 3]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(-v[0].copied().unwrap_or(0))
    })
    .unwrap();
    let leaves = [
        ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(SimpleScore::ZERO)),
        ScoredSearchTermination::Condition(ScoredSearchLimit::Search(
            SearchLimit::MoveEvaluations(2),
        )),
    ];
    let condition = ScoredSearchTermination::All(&leaves);
    let unused = SearchTermination::Condition(SearchLimit::Steps(100));
    let mut config = plan(&unused);
    config.limits.termination = None;
    config.limits.scored_termination = Some(&condition);
    let r = solve(&p, &n, config).unwrap();
    assert_eq!(r.work.move_evaluations, 2);
    assert_eq!(r.termination, SolverTermination::Condition);

    let token = CancellationToken::new();
    let calls = Cell::new(0);
    let calls_ref = &calls;
    let token_ref = &token;
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("cancel", vec![0], move |_| {
        calls_ref.set(calls_ref.get() + 1);
        if calls_ref.get() == 3 {
            token_ref.cancel();
        }
        SimpleScore::ZERO
    })
    .unwrap();
    let quota = SearchTermination::Condition(SearchLimit::MoveEvaluations(3));
    let mut config = plan(&quota);
    config.limits.cancellation = Some(&token);
    let metrics = SolverMetrics::new(None);
    let r = solve_with_extensions(
        &p,
        &n,
        config,
        SolverExtensions {
            metrics: Some(&metrics),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.termination, SolverTermination::Cancelled);
    assert_eq!(r.work.move_evaluations, 1);
    assert_eq!(r.work.completed_steps, 0);
    assert_eq!(
        metrics
            .snapshot()
            .latest
            .unwrap()
            .move_evaluations
            .get(MoveEvaluationKind::Reconstruction),
        1
    );
}

#[test]
fn failed_calculator_reconstruction_keeps_bulk_publication_transactional() {
    use std::{cell::Cell, rc::Rc};
    struct Failing {
        changes: Rc<Cell<usize>>,
        score: SimpleScore,
    }
    impl IncrementalCalculator<Assignment<i64>> for Failing {
        type Score = SimpleScore;
        type Justification = ();
        type Change = Vec<usize>;
        fn reset(
            &mut self,
            a: &Assignment<i64>,
            _: &mut MatchRegistry<SimpleScore, ()>,
        ) -> Result<(), MatchRegistryError> {
            self.score = a.objective();
            Ok(())
        }
        fn after_change(
            &mut self,
            a: &Assignment<i64>,
            _: &Vec<usize>,
            r: &mut MatchRegistry<SimpleScore, ()>,
        ) -> Result<(), MatchRegistryError> {
            let count = self.changes.get() + 1;
            self.changes.set(count);
            if count == 3 {
                return Err(MatchRegistryError::Calculator("later candidate".into()));
            }
            self.reset(a, r)
        }
        fn calculate_score(&mut self) -> SimpleScore {
            self.score
        }
    }
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2, 3]).with_initial(0)]).unwrap();
    let changes = Rc::new(Cell::new(0));
    let factory = || Failing {
        changes: changes.clone(),
        score: SimpleScore::ZERO,
    };
    let quota = SearchTermination::Condition(SearchLimit::MoveEvaluations(3));
    let metrics = SolverMetrics::new(None);
    let r = solve_calculator_with_extensions(
        &p,
        &factory,
        plan(&quota),
        SolverExtensions {
            metrics: Some(&metrics),
            ..Default::default()
        },
    );
    assert!(r.is_err());
    assert!(changes.get() >= 3);
    assert_eq!(
        metrics.snapshot().latest.unwrap().move_evaluations.total(),
        0
    );
    let r = solve_calculator_with_extensions(
        &p,
        &factory,
        plan(&quota),
        SolverExtensions {
            metrics: Some(&metrics),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.work.move_evaluations, 3);
    assert_eq!(r.termination, SolverTermination::Condition);
    assert_eq!(
        metrics
            .snapshot()
            .latest
            .unwrap()
            .move_evaluations
            .get(MoveEvaluationKind::Reconstruction),
        3
    );
}
