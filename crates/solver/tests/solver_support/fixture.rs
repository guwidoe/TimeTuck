use std::cell::Cell;
use timetuck::*;

#[derive(Debug, PartialEq)]
struct Outcome {
    values: Vec<Option<usize>>,
    score: Option<SimpleScore>,
    complete: bool,
    work: SolverWork,
    phases: Vec<PhaseReport<SimpleScore>>,
    reason: SolverTermination,
}
#[derive(Debug, PartialEq)]
enum Failure {
    Bound(usize),
    Empty,
    Incomplete(usize),
    Acceptance(usize),
    Token(usize),
    Phase(usize),
    Repair(usize),
}
fn plan() -> SolverConfig<'static> {
    SolverConfig {
        local_search: vec![
            LocalSearchConfig {
                max_steps: 1,
                ..Default::default()
            };
            2
        ],
        ..Default::default()
    }
}
fn run(
    mode: usize,
    initialized: bool,
    c: SolverConfig<'_>,
    order: MixedConstructionOrder,
    factories: &Cell<usize>,
    fail_factory: Option<usize>,
    hook: impl Fn(),
) -> Result<Outcome, Failure> {
    run_scored(
        (mode, initialized, order),
        c,
        factories,
        fail_factory,
        hook,
        SimpleScore::new,
    )
}
fn run_scored<'a>(
    scenario: (usize, bool, MixedConstructionOrder),
    c: impl Into<SolverSequence<'a>>,
    factories: &Cell<usize>,
    fail_factory: Option<usize>,
    hook: impl Fn(),
    make: impl Fn(i64) -> SimpleScore,
) -> Result<Outcome, Failure> {
    run_scored_observed(scenario, c, factories, fail_factory, hook, make, None)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Update {
    values: Vec<Option<usize>>,
    score: SimpleScore,
    complete: bool,
    phase: usize,
    kind: SolverPhaseKind,
}
fn run_scored_observed<'a>(
    scenario: (usize, bool, MixedConstructionOrder),
    c: impl Into<SolverSequence<'a>>,
    factories: &Cell<usize>,
    fail_factory: Option<usize>,
    hook: impl Fn(),
    make: impl Fn(i64) -> SimpleScore,
    observer: Option<&mut dyn FnMut(Update)>,
) -> Result<Outcome, Failure> {
    run_scored_events(
        scenario,
        c,
        factories,
        fail_factory,
        hook,
        make,
        observer,
        None,
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Initialized {
    values: Vec<Option<usize>>,
    score: Option<SimpleScore>,
    complete: bool,
    initialization: SolverInitialization,
}
#[allow(clippy::too_many_arguments)] // Shared six-model event fixture.
fn run_scored_events<'a>(
    scenario: (usize, bool, MixedConstructionOrder),
    c: impl Into<SolverSequence<'a>>,
    factories: &Cell<usize>,
    fail_factory: Option<usize>,
    hook: impl Fn(),
    make: impl Fn(i64) -> SimpleScore,
    observer: Option<&mut dyn FnMut(Update)>,
    initialization: Option<&mut dyn FnMut(Initialized)>,
) -> Result<Outcome, Failure> {
    run_scored_events_initial(
        scenario,
        c,
        factories,
        fail_factory,
        hook,
        make,
        observer,
        initialization,
        0,
    )
}
#[allow(clippy::too_many_arguments)] // Shared six-model event fixture.
fn run_scored_events_initial<'a>(
    scenario: (usize, bool, MixedConstructionOrder),
    c: impl Into<SolverSequence<'a>>,
    factories: &Cell<usize>,
    fail_factory: Option<usize>,
    hook: impl Fn(),
    make: impl Fn(i64) -> SimpleScore,
    mut observer: Option<&mut dyn FnMut(Update)>,
    mut initialization: Option<&mut dyn FnMut(Initialized)>,
    initial_value: usize,
) -> Result<Outcome, Failure> {
    let c = c.into();
    let (mode, initialized, order) = scenario;
    let score = |v: &[Option<usize>]| {
        hook();
        make(v.iter().flatten().map(|&v| v as i64).sum())
    };
    let factory = || {
        factories.set(factories.get() + 1);
        if fail_factory == Some(factories.get()) {
            Err(GraphError::ForeignPort)
        } else {
            Ok(())
        }
    };
    macro_rules! observing {
        ($plain:path, $observed:path, $events:path, $values:expr; $($args:expr),*) => {{
            if let Some(initialization) = initialization.as_deref_mut() {
                $events($($args,)* |event| match event {
                    SolverEvent::Best(u) => { if let Some(observer) = observer.as_deref_mut() {
                        observer(Update { values: $values(u.assignment), score: u.score, complete: u.complete, phase: u.phase, kind: u.kind });
                    } },
                    SolverEvent::FirstInitialized { initialization: info, assignment, score, complete } => {
                        initialization(Initialized { values: $values(assignment), score, complete, initialization: info });
                    }
                })
            } else if let Some(observer) = observer.as_deref_mut() {
                $observed($($args,)* |u| observer(Update {
                    values: $values(u.assignment), score: u.score, complete: u.complete,
                    phase: u.phase, kind: u.kind,
                }))
            } else { $plain($($args),*) }
        }};
    }
    macro_rules! outcome {
        ($r:expr,$p:expr,$values:expr) => {{
            match $r {
                Ok(r) => {
                    $p.validate(&r.assignment).unwrap();
                    assert_eq!(r.complete, $p.is_complete(&r.assignment));
                    let values: Vec<Option<usize>> = $values(&r.assignment);
                    assert_eq!(r.solution().is_some(), r.complete && r.score.is_some());
                    if let Some(s) = r.score {
                        assert_eq!(
                            s,
                            make(values.iter().flatten().map(|&v| v as i64).sum()).with_structural(
                                -(values.iter().filter(|v| v.is_none()).count() as i64)
                            )
                        );
                    }
                    Ok(Outcome {
                        values,
                        score: r.score,
                        complete: r.complete,
                        work: r.work,
                        phases: r.phases,
                        reason: r.termination,
                    })
                }
                Err(SolverError::Repair { phase, .. }) => Err(Failure::Repair(phase)),
                Err(SolverError::PinnedUnassignedVariable { .. }) => panic!("fixture uses permissive pin policy"),
                Err(SolverError::EmptyPlan) => Err(Failure::Empty),
                Err(SolverError::InitialScoring { .. }) => panic!("fixture models have movable entities"),
                Err(SolverError::InvalidMoveProviderPhase { .. }) => panic!("fixture has no provider bindings"),
                Err(SolverError::InvalidScoreTrend { .. }) => panic!("fixture has no trend"),
                Err(SolverError::PhaseScoreMismatch { .. } | SolverError::PhaseScoreCheckFailed { .. }) => {
                    panic!("fixture has no phase score checker")
                }
                Err(SolverError::InvalidOptimisticBound { phase, .. }) => {
                    Err(Failure::Bound(phase))
                }
                Err(SolverError::IncompleteLocalSearch { phase }) => {
                    Err(Failure::Incomplete(phase))
                }
                Err(SolverError::InvalidAcceptance { phase, .. }) => {
                    Err(Failure::Acceptance(phase))
                }
                Err(SolverError::ConflictingCancellation { phase }) => Err(Failure::Token(phase)),
                Err(SolverError::Phase { phase, .. }) => Err(Failure::Phase(phase)),
                Err(SolverError::Custom { .. } | SolverError::UnboundCustomPhase { .. }) => {
                    panic!("fixture has no bound custom phases")
                }
            }
        }};
    }
    let v = Variable::new(vec![0usize, 1, 2]);
    let basic = FiniteProblem::new(vec![if initialized {
        v.with_initial(initial_value)
    } else {
        v
    }])
    .unwrap();
    if mode < 2 {
        let values = |a: &Assignment<usize>| a.values().to_vec();
        if mode == 0 {
            let mut n = ConstraintNetwork::new(1);
            n.add_constraint("score", vec![0], |v| score(&[v[0].copied()]))
                .unwrap();
            return outcome!(
                observing!(solve, solve_with_observer, solve_with_events, values; &basic, &n, c),
                basic,
                values
            );
        }
        return outcome!(
            observing!(solve_stream, solve_stream_with_observer, solve_stream_with_events, values;
                &basic,
                || {
                    factory()?;
                    let mut b = StreamGraphBuilder::new();
                    let s = b.source::<usize, (usize, Option<usize>)>();
                    b.score("score", s.port(), |(_, v)| score(&[*v]))?;
                    Ok((b.build(), s))
                },
                c),
            basic,
            values
        );
    }
    let lists = ListProblem::new(
        vec![7usize],
        (0..3)
            .map(|i| {
                ListEntity::new(if initialized && i == initial_value {
                    vec![7]
                } else {
                    vec![]
                })
            })
            .collect(),
    )
    .unwrap();
    if mode < 4 {
        let values = |a: &ListAssignment<usize>| vec![a.location(&7).map(|p| p.entity)];
        if mode == 2 {
            let mut n = ListConstraintNetwork::new(3, 1);
            n.add_constraint("score", vec![ListScope::Value(0)], |v| {
                let ListFact::Value { element, .. } = &v[0] else {
                    panic!()
                };
                score(&[element.as_ref().map(|v| v.position.entity)])
            })
            .unwrap();
            return outcome!(
                observing!(solve_list, solve_list_with_observer, solve_list_with_events, values; &lists, &n, c),
                lists,
                values
            );
        }
        return outcome!(
            observing!(solve_list_stream, solve_list_stream_with_observer, solve_list_stream_with_events, values;
                &lists,
                || {
                    factory()?;
                    let mut b = StreamGraphBuilder::new();
                    let s = ListStreamSources::<usize>::new(&mut b);
                    b.score("score", s.values.port(), |v| {
                        score(&[v.position.map(|p| p.entity)])
                    })?;
                    Ok((b.build(), s))
                },
                c),
            lists,
            values
        );
    }
    let p = MixedProblem::new(basic, lists);
    let values = |a: &MixedAssignment<usize, usize>| {
        vec![
            a.basic().values()[0],
            a.lists().location(&7).map(|p| p.entity),
        ]
    };
    let c = MixedSolverSequence {
        limits: c.limits,
        phases: c
            .phases
            .into_iter()
            .map(|phase| match phase {
                SolverPhase::Construction(construction) => {
                    MixedSolverPhase::Construction(MixedConstructionConfig {
                        construction,
                        order,
                    })
                }
                SolverPhase::LocalSearch(search) => {
                    MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                        search,
                        joint_moves: true,
                    })
                }
                SolverPhase::RuinRecreate(search) => {
                    MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig { search, order })
                }
                SolverPhase::Exact(c) => MixedSolverPhase::Exact(c),
                SolverPhase::Custom(c) => MixedSolverPhase::Custom(c),
            })
            .collect(),
    };
    if mode == 4 {
        let mut n = MixedConstraintNetwork::new(1, 3, 1);
        n.add_constraint(
            "score",
            vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
            |v| {
                let (MixedFact::Basic(a), MixedFact::ListValue { element, .. }) = (&v[0], &v[1])
                else {
                    panic!()
                };
                score(&[a.copied(), element.as_ref().map(|v| v.position.entity)])
            },
        )
        .unwrap();
        return outcome!(
            observing!(solve_mixed, solve_mixed_with_observer, solve_mixed_with_events, values; &p, &n, c),
            p,
            values
        );
    }
    outcome!(
        observing!(solve_mixed_stream, solve_mixed_stream_with_observer, solve_mixed_stream_with_events, values;
            &p,
            || {
                factory()?;
                let mut b = StreamGraphBuilder::new();
                let s = MixedStreamSources::<usize, usize>::new(&mut b);
                let j = b.join_equal(s.basic.port(), s.lists.values.port(), |_| (), |_| ())?;
                b.score("score", &j, |((_, a), v)| {
                    score(&[*a, v.position.map(|p| p.entity)])
                })?;
                Ok((b.build(), s))
            },
            c),
        p,
        values
    )
}
fn execute(mode: usize, c: SolverConfig<'_>) -> Outcome {
    let factories = Cell::new(0);
    let callbacks = Cell::new(0);
    let r = run(
        mode,
        true,
        c,
        MixedConstructionOrder::BasicThenLists,
        &factories,
        None,
        || callbacks.set(callbacks.get() + 1),
    )
    .unwrap();
    if mode % 2 == 1 {
        assert_eq!(factories.get(), r.phases.len());
    } else {
        assert_eq!(factories.get(), 0);
    }
    assert_eq!(callbacks.get() as u128, r.work.constraint_evaluations);
    assert_eq!(
        r.work.evaluations,
        r.phases.iter().map(|p| p.work.evaluations).sum()
    );
    assert_eq!(
        r.work.move_evaluations,
        r.phases.iter().map(|p| p.work.move_evaluations).sum()
    );
    assert_eq!(
        r.work.completed_steps,
        r.phases.iter().map(|p| p.work.completed_steps).sum()
    );
    assert_eq!(
        r.work.accepted_moves,
        r.phases.iter().map(|p| p.work.accepted_moves).sum()
    );
    for (index, p) in r.phases.iter().enumerate() {
        assert_eq!(p.index, index);
    }
    r
}
