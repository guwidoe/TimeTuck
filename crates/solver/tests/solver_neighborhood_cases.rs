// Adapted from pinned Community DefaultSolverTest and testdomain fixtures.
// Apache-2.0; exact source identities and native substitutions: compat/provenance.json.
use timetuck::{variable_neighborhood_descent, *};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Value {
    Basic(usize),
    Other(usize),
    Lead(usize),
}
fn plan<'a>(union: Option<&'a NeighborhoodUnion<'a>>) -> SolverSequence<'a, SimpleScore> {
    // Initialized source fixtures enter local search directly. Explicit native
    // seeds/budgets replace Java default search settings; preserve the score goal.
    SolverSequence {
        limits: SolverLimits {
            best_score_limit: Some(SimpleScore::ZERO),
            ..Default::default()
        },
        phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
            neighborhood_union: union,
            acceptance: Acceptance::HillClimbing,
            move_order: MoveOrder::ShuffledMoves {
                seed: 20261007,
                window: 64.try_into().unwrap(),
            },
            max_steps: 200,
            max_move_evaluations: Some(10_000),
            assert_scores: true,
            ..Default::default()
        })],
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Default,
    ExtraSwap,
    Descent,
}
fn basic(p: &FiniteProblem<Value>, objective: fn(&[Option<Value>]) -> SimpleScore, mode: Mode) {
    let original = p.initial_assignment();
    let union_config = NeighborhoodUnion {
        // Preserve default change/swap candidates plus an explicit swap selector.
        children: &[(Neighborhood::ChangeAndSwap, 1), (Neighborhood::Swap, 1)],
        order: UnionOrder::Sequential,
    };
    for stream in [false, true] {
        let descent = NeighborhoodUnion {
            children: &[(Neighborhood::Change, 1), (Neighborhood::Swap, 1)],
            order: UnionOrder::Sequential,
        };
        let mut config = plan((mode == Mode::ExtraSwap).then_some(&union_config));
        if mode == Mode::Descent {
            config.phases = vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 200,
                max_move_evaluations: Some(10_000),
                assert_scores: true,
                ..variable_neighborhood_descent(&descent).unwrap()
            })];
        }
        let mut events = 0;
        let mut observe = |e: SolverEvent<'_, Assignment<Value>, SimpleScore>| {
            if matches!(e, SolverEvent::Best(_)) {
                events += 1;
            }
        };
        let r = if stream {
            solve_stream_with_events(
                p,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let source = b.source::<usize, (usize, Option<Value>)>();
                    let count = p.variables().len();
                    let all = b.collect_list(source.port(), |row| *row)?;
                    b.score("test objective", &all, move |rows| {
                        // Graph initialization/replay inserts source rows individually.
                        // Preserve field positions even while only a prefix exists.
                        let mut values = vec![None; count];
                        for (index, value) in rows.iter() {
                            values[*index] = *value;
                        }
                        objective(&values)
                    })?;
                    Ok((b.build(), source))
                },
                config,
                &mut observe,
            )
            .unwrap()
        } else {
            let mut n = ConstraintNetwork::new(p.variables().len());
            n.add_constraint(
                "test objective",
                (0..p.variables().len()).collect(),
                |rows| objective(&rows.iter().map(|v| v.copied()).collect::<Vec<_>>()),
            )
            .unwrap();
            solve_with_events(p, &n, config, &mut observe).unwrap()
        };
        // PlannerTestUtils requires a best-solution event and non-null solution.
        // The original score-limit0 fixture also requires finding the target.
        assert!(events > 0 && r.complete && r.solution().is_some());
        assert_eq!(r.score, Some(SimpleScore::ZERO));
        assert_eq!(objective(r.assignment.values()), SimpleScore::ZERO);
        assert!(r.work.move_evaluations > 0 && r.work.move_evaluations <= 10_000);
        p.validate(&r.assignment).unwrap();
        assert_eq!(p.initial_assignment(), original);
    }
}
fn basic_problem() -> FiniteProblem<Value> {
    // TestdataSolution.generateSolution(valueListSize=3, entityListSize=2).
    FiniteProblem::new(
        (0..2)
            .map(|i| {
                Variable::new((0..3).map(Value::Basic).collect()).with_initial(Value::Basic(i))
            })
            .collect(),
    )
    .unwrap()
}
fn basic_score(values: &[Option<Value>]) -> SimpleScore {
    // The source penalizes distinct occurrences of the FIRST value, not all
    // repetitions or the number of entities using it.
    SimpleScore::new(-i64::from(values.contains(&Some(Value::Basic(0)))))
}
#[test]
fn solve_with_neighborhoods() {
    basic(&basic_problem(), basic_score, Mode::Default);
}
#[test]
fn solve_with_default_neighborhood_provider() {
    basic(&basic_problem(), basic_score, Mode::Default);
}
#[test]
fn solve_with_neighborhoods_and_move_selectors() {
    basic(&basic_problem(), basic_score, Mode::ExtraSwap);
}
#[test]
fn solve_with_default_neighborhood_provider_multi_var() {
    // Two entities, two required fields and one optional field per entity.
    let p = FiniteProblem::new(
        (0..2)
            .flat_map(|e| {
                [
                    Variable::new((0..3).map(Value::Basic).collect()).with_initial(Value::Basic(e)),
                    Variable::new((0..3).map(Value::Basic).collect())
                        .with_initial(Value::Basic(e + 1)),
                    Variable::new((0..3).map(Value::Other).collect())
                        .allowing_unassigned()
                        .with_initial(Value::Other(e)),
                ]
            })
            .collect(),
    )
    .unwrap();
    basic(
        &p,
        |v| {
            let primary = v.chunks_exact(3).any(|e| e[0] == Some(Value::Basic(0)));
            let secondary = v.chunks_exact(3).any(|e| e[1] == Some(Value::Basic(1)));
            let tertiary = v.chunks_exact(3).any(|e| e[2] == Some(Value::Other(0)));
            SimpleScore::new(-i64::from(primary) - i64::from(secondary) - i64::from(tertiary))
        },
        Mode::Default,
    );
}
#[test]
fn solve_with_default_neighborhood_provider_multi_entity() {
    let mut vars = vec![
        Variable::new(vec![Value::Basic(0), Value::Basic(1)])
            .with_initial(Value::Basic(0));
        2
    ];
    vars.extend(vec![
        Variable::new(vec![Value::Lead(0), Value::Lead(1)])
            .with_initial(Value::Lead(0));
        2
    ]);
    let p = FiniteProblem::new(vars).unwrap();
    basic(
        &p,
        |v| {
            SimpleScore::new(
                -i64::from(v[..2].contains(&Some(Value::Basic(0))))
                    - i64::from(v[2..].contains(&Some(Value::Lead(0)))),
            )
        },
        Mode::Default,
    );
}
#[test]
fn solve_with_default_neighborhood_provider_list_var() {
    let p = ListProblem::new(
        vec![0, 1],
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![])],
    )
    .unwrap();
    fn penalty(size: usize) -> SimpleScore {
        SimpleScore::new(-(size.saturating_sub(1).pow(2) as i64))
    }
    for stream in [false, true] {
        let mut events = 0;
        let mut observe = |e: SolverEvent<'_, ListAssignment<i32>, SimpleScore>| {
            if matches!(e, SolverEvent::Best(_)) {
                events += 1;
            }
        };
        let r = if stream {
            solve_list_stream_with_events(
                &p,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let sources = ListStreamSources::new(&mut b);
                    b.score("list size", sources.entities.port(), |(_, list)| {
                        penalty(list.len())
                    })?;
                    Ok((b.build(), sources))
                },
                plan(None),
                &mut observe,
            )
            .unwrap()
        } else {
            let mut n = ListConstraintNetwork::new(2, 2);
            for e in 0..2 {
                n.add_constraint(format!("list size {e}"), vec![ListScope::Entity(e)], |f| {
                    let ListFact::Entity(list) = f[0] else {
                        panic!()
                    };
                    penalty(list.len())
                })
                .unwrap();
            }
            solve_list_with_events(&p, &n, plan(None), &mut observe).unwrap()
        };
        assert!(events > 0 && r.complete && r.solution().is_some());
        assert_eq!(r.score, Some(SimpleScore::ZERO));
        assert!(r.assignment.lists().iter().all(|list| list.len() == 1));
        assert!(r.work.move_evaluations > 0 && r.work.move_evaluations <= 10_000);
        p.validate(&r.assignment).unwrap();
        assert_eq!(p.initial_assignment().lists(), &[vec![0, 1], vec![]]);
    }
}

#[test]
fn variable_neighborhood_descent_still_works_without_neighborhoods() {
    basic(&basic_problem(), basic_score, Mode::Descent);
}
