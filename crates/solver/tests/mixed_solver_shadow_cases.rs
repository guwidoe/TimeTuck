// Adapted from pinned Timefold Community DefaultSolverTest and mixed test fixtures.
// Apache-2.0; identities and native substitutions: compat/provenance.json.
use std::collections::BTreeMap;
use timetuck::*;

type Model = MixedProblem<usize, usize>;
type State = MixedAssignment<usize, usize>;
type Graph = StreamGraph<'static, SimpleScore>;
type ReadShadows = Box<dyn Fn(&StreamGraph<'_, SimpleScore>) -> Shadows>;
#[derive(Clone, Debug, PartialEq, Eq)]
struct Shadows {
    basic: BTreeMap<usize, Option<i64>>,
    cascade: BTreeMap<usize, Option<usize>>,
    declarative: BTreeMap<usize, Option<usize>>,
    inverse: BTreeMap<usize, usize>,
}
#[derive(Clone, Copy)]
struct Shape {
    entities: usize,
    values: usize,
    others: usize,
}
fn model(s: Shape, reversed: bool) -> Model {
    let basic = (0..2 * s.entities)
        .map(|i| {
            let v = Variable::new((0..s.others).collect());
            if reversed && i % 2 == 0 {
                v.with_initial(1 - i / 2)
            } else {
                v
            }
        })
        .collect();
    let lists = (0..s.entities)
        .map(|e| {
            ListEntity::new(if reversed && e == 0 {
                vec![0, 1]
            } else {
                vec![]
            })
        })
        .collect();
    MixedProblem::new(
        FiniteProblem::new(basic).unwrap(),
        ListProblem::new((0..s.values).collect(), lists).unwrap(),
    )
}
fn graph(
    s: Shape,
    neighborhood_objective: bool,
    prune: bool,
) -> Result<(Graph, MixedStreamSources<usize, usize>, ReadShadows), GraphError> {
    let mut b = if prune {
        StreamGraphBuilder::new().with_activity_pruning()
    } else {
        StreamGraphBuilder::new()
    };
    let sources = MixedStreamSources::<usize, usize>::new(&mut b);
    let first = b.filter(sources.basic.port(), |r| r.0 % 2 == 0)?;
    // First-basic declarative shadow: the immutable referenced value's strength.
    let basic_shadow = b.map(&first, move |r| {
        (r.0 / 2, r.1.map(|v| s.values as i64 - v as i64))
    })?;
    let basic_view = b.dataset(&basic_shadow)?;
    // These specific upstream cascade/declarative formulas depend only on index.
    // Graph maps update them transactionally alongside built-in list shadow rows.
    let cascade = b.map(sources.lists.values.port(), |r| {
        (r.value, r.position.map(|p| p.index + 1))
    })?;
    let declarative = b.map(sources.lists.values.port(), |r| {
        (r.value, r.position.map(|p| p.index + 2))
    })?;
    let cascade_view = b.dataset(&cascade)?;
    let declarative_view = b.dataset(&declarative)?;
    let assigned = b.filter(&first, |r| r.1.is_some())?;
    let counts =
        b.group_collect_with::<_, _, _, _, CountCollector>(&assigned, |r| (r.1.unwrap(), ()))?;
    let value_facts = b.source::<usize, usize>();
    let all_counts = b.complement(&counts, value_facts.port(), |r| r.0, |v| *v, |v| (*v, 0))?;
    let inverse = b.map(&all_counts, |r| (r.0, r.1 + 2))?;
    let inverse_view = b.dataset(&inverse)?;
    b.score("basics", sources.basic.port(), move |r| {
        SimpleScore::new(if neighborhood_objective {
            -i64::from(r.1 == Some(if r.0 % 2 == 0 { 0 } else { 1 }))
        } else {
            i64::from(r.1.is_some())
        })
    })?;
    b.score("lists", sources.lists.entities.port(), move |r| {
        SimpleScore::new(if neighborhood_objective {
            -(r.1.len().saturating_sub(1) as i64).pow(2)
        } else if r.1.len() == 1 {
            2
        } else {
            1
        })
    })?;
    let mut graph = b.build();
    let mut tx = graph.transaction();
    for v in 0..s.others {
        tx.apply(&value_facts, TupleChange::put(v, v))?;
    }
    tx.commit()?;
    let read: ReadShadows = Box::new(move |g| Shadows {
        basic: g
            .dataset_rows(&basic_view)
            .unwrap()
            .values()
            .copied()
            .collect(),
        cascade: g
            .dataset_rows(&cascade_view)
            .unwrap()
            .values()
            .copied()
            .collect(),
        declarative: g
            .dataset_rows(&declarative_view)
            .unwrap()
            .values()
            .copied()
            .collect(),
        inverse: g
            .dataset_rows(&inverse_view)
            .unwrap()
            .values()
            .copied()
            .collect(),
    });
    Ok((graph, sources, read))
}
// Independent fixture arithmetic over lists/assignments, without derived row APIs.
fn expected(s: Shape, a: &State, objective: bool) -> (SimpleScore, Shadows) {
    let mut shadows = Shadows {
        basic: BTreeMap::new(),
        cascade: (0..s.values).map(|v| (v, None)).collect(),
        declarative: (0..s.values).map(|v| (v, None)).collect(),
        inverse: (0..s.others).map(|v| (v, 2)).collect(),
    };
    let mut score = 0;
    let mut missing = 0;
    for e in 0..s.entities {
        let first = a.basic().values()[2 * e];
        let second = a.basic().values()[2 * e + 1];
        shadows
            .basic
            .insert(e, first.map(|v| s.values as i64 - v as i64));
        if let Some(v) = first {
            *shadows.inverse.get_mut(&v).unwrap() += 1;
        }
        missing += i64::from(first.is_none()) + i64::from(second.is_none());
        let list = &a.lists().lists()[e];
        if objective {
            score -= i64::from(first == Some(0)) + i64::from(second == Some(1));
            if list.len() > 1 {
                let excess = list.len() as i64 - 1;
                score -= excess * excess;
            }
        } else {
            score += i64::from(first.is_some())
                + i64::from(second.is_some())
                + if list.len() == 1 { 2 } else { 1 };
        }
        for (index, &v) in list.iter().enumerate() {
            shadows.cascade.insert(v, Some(index + 1));
            shadows.declarative.insert(v, Some(index + 2));
        }
    }
    missing += shadows.cascade.values().filter(|v| v.is_none()).count() as i64;
    (SimpleScore::new(score).with_structural(-missing), shadows)
}
fn plan(objective: bool) -> MixedSolverConfig<'static> {
    MixedSolverConfig {
        limits: SolverLimits {
            best_score_limit: objective.then_some(SimpleScore::ZERO),
            ..Default::default()
        },
        local_search: vec![MixedLocalSearchConfig {
            search: LocalSearchConfig {
                max_steps: 16,
                move_order: MoveOrder::Original,
                ..Default::default()
            },
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn solve_case(s: Shape, objective: bool, prune: bool) {
    let p = model(s, objective);
    let mut events = 0;
    let result = solve_mixed_stream_with_observer(
        &p,
        || {
            let (g, s, _) = graph(s, objective, prune)?;
            Ok((g, s))
        },
        plan(objective),
        |u| {
            events += 1;
            assert_eq!(u.score, expected(s, u.assignment, objective).0);
        },
    )
    .unwrap();
    assert!(events > 0); // PlannerTestUtils.solve helper assertion.
    assert!(result.complete);
    assert!(
        result
            .assignment
            .basic()
            .values()
            .iter()
            .all(Option::is_some)
    );
    assert_eq!(
        result
            .assignment
            .lists()
            .lists()
            .iter()
            .filter(|l| l.is_empty())
            .count(),
        s.entities.saturating_sub(s.values)
    );
    let (score, shadows) = expected(s, &result.assignment, objective);
    assert_eq!(result.score, Some(score));
    if objective {
        assert_eq!(score, SimpleScore::ZERO);
    }
    // Public solve returns assignment+score. Materialize final derived output using
    // the same real graph factory; no Java mutable-object shadow lifecycle claimed.
    let (g, sources, read) = graph(s, objective, prune).unwrap();
    let mut d = MixedStreamScoreDirector::new(&p, g, sources, result.assignment).unwrap();
    assert_eq!(d.score(), score);
    assert_eq!(read(d.graph()), shadows);
    d.assert_consistent().unwrap();
}
#[test]
fn solve_mixed_model() {
    for (entities, values, others) in [(2, 2, 2), (2, 3, 2), (2, 2, 3), (3, 2, 2)] {
        for prune in [false, true] {
            solve_case(
                Shape {
                    entities,
                    values,
                    others,
                },
                false,
                prune,
            );
        }
    }
}
#[test]
fn solve_with_default_neighborhood_provider_mixed_model() {
    for prune in [false, true] {
        solve_case(
            Shape {
                entities: 2,
                values: 2,
                others: 2,
            },
            true,
            prune,
        );
    }
}

#[test]
fn original_mixed_shadow_trials_restore_empty_inverse_groups_and_all_derived_rows() {
    let s = Shape {
        entities: 3,
        values: 5,
        others: 3,
    };
    let p = model(s, false);
    for prune in [false, true] {
        let (g, sources, read) = graph(s, false, prune).unwrap();
        let mut d = MixedStreamScoreDirector::new(&p, g, sources, p.initial_assignment()).unwrap();
        let mut seed = 631_u64;
        for step in 0..240 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let old = d.assignment().clone();
            let held = read(d.graph());
            let mut moves = mixed_moves(&p, &old, Neighborhood::ChangeAndSwap, true)
                .unwrap()
                .collect::<Vec<_>>();
            // Required variables may be cleared in a valid partial trial; force
            // transitions absent from complete-solution improvement neighborhoods.
            moves.push(MixedMove {
                basic: Move::Change {
                    variable: step % (2 * s.entities),
                    value: None,
                },
                lists: ListMove::Composite(vec![]),
            });
            for e in 0..s.entities {
                let len = old.lists().lists()[e].len();
                if len > 0 {
                    moves.push(MixedMove {
                        basic: Move::Composite(vec![]),
                        lists: ListMove::Unassign {
                            entity: e,
                            start: 0,
                            end: len,
                        },
                    });
                }
            }
            let movement = moves.swap_remove((seed as usize) % moves.len());
            let mut independent = old.clone();
            p.apply(&mut independent, &movement).unwrap().commit();
            let trial = d.apply(&movement).unwrap();
            let (score, shadows) = expected(s, &independent, false);
            assert_eq!(trial.assignment(), &independent);
            assert_eq!(trial.score(), score);
            assert_eq!(read(trial.graph()), shadows);
            if step % 3 == 0 {
                drop(trial);
                assert_eq!(d.assignment(), &old);
                assert_eq!(read(d.graph()), held);
            } else {
                trial.commit();
            }
            let (score, shadows) = expected(s, d.assignment(), false);
            assert_eq!(d.score(), score);
            assert_eq!(read(d.graph()), shadows);
        }
    }
}

fn custom_case(command_phase: bool) {
    let s = Shape {
        entities: 2,
        values: 2,
        others: 2,
    };
    let p = model(s, false);
    let mut called = 0;
    let mut proposed = 0;
    let mut best_events = 0;
    let movement = |value| MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(value),
        },
        lists: ListMove::Composite(vec![]),
    };
    let mut command = |index,
                       ctx: &mut CustomPhaseContext<
        '_,
        State,
        MixedMove<usize, usize>,
        SimpleScore,
        MixedStreamScoringError,
    >| {
        assert_eq!(index, 1);
        called += 1;
        // MixedCustomPhaseCommand executes the iterator's first proposal.
        assert!(ctx.execute(&movement(0))?.is_some());
        Ok(())
    };
    let mut provider = |_: &Model, _: &State, _: u64| {
        proposed += 1;
        // Exact source factory order; its RandomGenerator argument is unused.
        vec![movement(0), movement(1)]
    };
    let mut events = |event: SolverEvent<'_, State, SimpleScore>| {
        if let SolverEvent::Best(u) = event {
            best_events += 1;
            assert_eq!(u.score, expected(s, u.assignment, false).0);
        }
    };
    let mut extensions = MixedSolverExtensions {
        events: Some(&mut events),
        ..Default::default()
    };
    if command_phase {
        extensions.command = Some(&mut command);
    } else {
        extensions.move_providers.bind(1, &mut provider);
    }
    let phase = if command_phase {
        MixedSolverPhase::Custom(CustomPhaseConfig {
            max_move_evaluations: Some(16),
            ..Default::default()
        })
    } else {
        MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
            search: LocalSearchConfig {
                max_steps: 16,
                move_order: MoveOrder::Original,
                ..Default::default()
            },
            ..Default::default()
        })
    };
    let result = solve_mixed_stream_with_extensions(
        &p,
        || {
            let (g, sources, _) = graph(s, false, false)?;
            Ok((g, sources))
        },
        MixedSolverSequence {
            phases: vec![
                MixedSolverPhase::Construction(MixedConstructionConfig::default()),
                phase,
            ],
            ..Default::default()
        },
        extensions,
    )
    .unwrap();
    assert!(result.complete);
    assert!(best_events > 0);
    if command_phase {
        assert_eq!(called, 1);
    } else {
        assert!(proposed > 0);
    }
    assert!(
        result
            .assignment
            .basic()
            .values()
            .iter()
            .all(Option::is_some)
    );
    assert!(
        result
            .assignment
            .lists()
            .lists()
            .iter()
            .all(|l| !l.is_empty())
    );
    let (score, shadows) = expected(s, &result.assignment, false);
    assert_eq!(result.score, Some(score));
    let (g, sources, read) = graph(s, false, false).unwrap();
    let d = MixedStreamScoreDirector::new(&p, g, sources, result.assignment).unwrap();
    assert_eq!(read(d.graph()), shadows);
}
#[test]
fn solve_mixed_model_custom_phase() {
    custom_case(true);
}
#[test]
fn solve_mixed_model_custom_move() {
    custom_case(false);
}
