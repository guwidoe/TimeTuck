// Original finite-planning/stream integration regressions; no upstream credit.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::{
    Assignment, FiniteProblem, GraphError, HardSoftScore, ModelError, Move, PlanningStreamSource,
    SimpleScore, StreamGraph, StreamGraphBuilder, StreamScoreDirector, StreamScoringError,
    TupleChange as Change, Variable, change_moves, exhaustive_search, swap_moves,
};

const WEIGHTS: [i64; 4] = [3, 2, 2, 1];
const PREFERRED: [usize; 4] = [0, 1, 1, 0];
const CONFLICTS: [(usize, usize); 4] = [(0, 1), (0, 2), (1, 3), (2, 3)];
fn network(
    calls: Rc<Cell<usize>>,
) -> (
    StreamGraph<'static, HardSoftScore>,
    PlanningStreamSource<usize>,
) {
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<usize>)>();
    let present = b
        .filter(source.port(), move |(_, value)| {
            calls.set(calls.get() + 1);
            assert!(value.is_none_or(|v| v < 2));
            value.is_some()
        })
        .unwrap();
    let assigned = b
        .map(&present, |&(id, value)| (id, value.unwrap()))
        .unwrap();
    let groups = b
        .group_collect(&assigned, |&(id, machine)| (machine, WEIGHTS[id]))
        .unwrap();
    b.score("capacity", &groups, |(_, s)| {
        HardSoftScore::new(-(s.sum - 4).max(0), 0)
    })
    .unwrap();
    let joined = b
        .join_equal(&assigned, &assigned, |row| row.1, |row| row.1)
        .unwrap();
    let conflicts = b
        .filter(&joined, |(a, b)| {
            a.0 < b.0 && CONFLICTS.contains(&(a.0, b.0))
        })
        .unwrap();
    b.score("conflicts", &conflicts, |_| HardSoftScore::new(-1, 0))
        .unwrap();
    b.score("preference", &assigned, |&(id, machine)| {
        HardSoftScore::new(0, -i64::from(machine != PREFERRED[id]))
    })
    .unwrap();
    (b.build(), source)
}
fn reference(values: &[Option<usize>]) -> BTreeMap<&'static str, HardSoftScore> {
    let mut loads = [0; 2];
    let mut preference = 0;
    let mut conflicts = 0;
    for (id, value) in values.iter().enumerate() {
        if let Some(machine) = value {
            loads[*machine] += WEIGHTS[id];
            preference -= i64::from(*machine != PREFERRED[id]);
        }
    }
    for (a, b) in CONFLICTS {
        if values[a].is_some() && values[a] == values[b] {
            conflicts -= 1;
        }
    }
    BTreeMap::from([
        (
            "capacity",
            HardSoftScore::new(-(loads[0] - 4).max(0) - (loads[1] - 4).max(0), 0),
        ),
        ("conflicts", HardSoftScore::new(conflicts, 0)),
        ("preference", HardSoftScore::new(0, preference)),
    ])
}
fn total(values: &[Option<usize>]) -> HardSoftScore {
    reference(values)
        .values()
        .fold(HardSoftScore::ZERO, |s, &v| s + v)
        .with_structural(-(values.iter().filter(|v| v.is_none()).count() as i64))
}

#[test]
fn every_partial_assignment_and_composite_undo_preserve_graph_and_model_state() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 4]).unwrap();
    let calls = Rc::new(Cell::new(0));
    let (graph, source) = network(calls.clone());
    let inspection = source.clone();
    let mut director =
        StreamScoreDirector::new(&problem, graph, source, problem.initial_assignment()).unwrap();
    assert_eq!(director.score(), HardSoftScore::ZERO.with_structural(-4));
    for state in 0..81 {
        let mut digits = state;
        let values: Vec<_> = (0..4)
            .map(|_| {
                let v = digits % 3;
                digits /= 3;
                match v {
                    0 => None,
                    1 => Some(0),
                    _ => Some(1),
                }
            })
            .collect();
        // Intermediate invalid value is restored by the composite. Only final
        // states are validated/scored and each touched index is propagated once.
        let movement = Move::Composite(
            std::iter::once(Move::Change {
                variable: 0,
                value: Some(9),
            })
            .chain(
                values
                    .iter()
                    .enumerate()
                    .map(|(variable, &value)| Move::Change { variable, value }),
            )
            .collect(),
        );
        let before = director.assignment().clone();
        let before_score = director.score();
        let before_parts = director
            .constraint_scores()
            .map(|(k, v)| (k.to_owned(), v))
            .collect::<BTreeMap<_, _>>();
        let before_calls = calls.get();
        {
            let candidate = director.apply(&movement).unwrap();
            assert_eq!(candidate.score(), total(&values));
            assert_eq!(
                candidate
                    .graph()
                    .constraint_scores()
                    .collect::<BTreeMap<_, _>>(),
                reference(&values)
            );
            assert_eq!(calls.get(), before_calls + 4);
            for (id, row) in candidate.graph().source_rows(&inspection).unwrap() {
                assert_eq!(*row, (*id, values[*id]));
            }
        }
        assert_eq!(calls.get(), before_calls + 4);
        assert_eq!(director.assignment(), &before);
        assert_eq!(director.score(), before_score);
        assert_eq!(
            director
                .constraint_scores()
                .map(|(k, v)| (k.to_owned(), v))
                .collect::<BTreeMap<_, _>>(),
            before_parts
        );
        let candidate = director.apply(&movement).unwrap();
        let count = calls.get();
        candidate.commit();
        assert_eq!(calls.get(), count);
        assert_eq!(director.assignment().values(), values);
        assert_eq!(director.score(), total(&values));
    }
}

#[test]
fn domains_pins_optional_null_and_invalid_indices_use_existing_model_validation() {
    let problem = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
        Variable::new(vec![0]),
        Variable::new(vec![0, 1]),
        Variable::new(vec![]).allowing_unassigned(),
    ])
    .unwrap();
    let calls = Rc::new(Cell::new(0));
    let (graph, source) = network(calls.clone());
    let mut director =
        StreamScoreDirector::new(&problem, graph, source, problem.initial_assignment()).unwrap();
    assert_eq!(director.score().structural(), -2);
    let count = calls.get();
    let original = director.assignment().clone();
    let score = director.score();
    for movement in [
        Move::Change {
            variable: 0,
            value: Some(1),
        },
        Move::Change {
            variable: 1,
            value: Some(1),
        },
        Move::Composite(vec![
            Move::Change {
                variable: 2,
                value: Some(1),
            },
            Move::Change {
                variable: 99,
                value: Some(0),
            },
        ]),
        Move::Change {
            variable: 3,
            value: Some(0),
        },
    ] {
        assert!(matches!(
            director.apply(&movement),
            Err(StreamScoringError::Model(_))
        ));
        assert_eq!(calls.get(), count);
        assert_eq!(director.assignment(), &original);
        assert_eq!(director.score(), score);
    }
    director
        .apply(&Move::Composite(vec![
            Move::Change {
                variable: 1,
                value: Some(0),
            },
            Move::Change {
                variable: 2,
                value: Some(1),
            },
        ]))
        .unwrap()
        .commit();
    assert_eq!(director.score().structural(), 0);
    assert_eq!(director.assignment().values()[3], None);
    let before = director.score();
    {
        let _candidate = director.apply(&Move::Swap { left: 1, right: 2 });
    }
    assert_eq!(director.score(), before);
}

#[test]
fn stream_failure_after_an_earlier_variable_update_rolls_back_the_whole_move() {
    let problem =
        FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2, 3]).with_initial(0); 2]).unwrap();
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<i64>)>();
    let inspect = source.clone();
    b.score("first", source.port(), |(_, v)| {
        SimpleScore::new(v.unwrap_or(0))
    })
    .unwrap();
    b.score("second", source.port(), |&(id, v)| {
        if id == 1 {
            assert_ne!(v, Some(3), "callback panic");
            if v == Some(2) {
                return SimpleScore::ZERO.with_structural(-1);
            }
        }
        SimpleScore::new(v.unwrap_or(0))
    })
    .unwrap();
    let mut director =
        StreamScoreDirector::new(&problem, b.build(), source, problem.initial_assignment())
            .unwrap();
    for bad in [2, 3] {
        let movement = Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: Some(1),
            },
            Move::Change {
                variable: 1,
                value: Some(bad),
            },
        ]);
        if bad == 2 {
            assert!(matches!(
                director.apply(&movement),
                Err(StreamScoringError::Graph(
                    GraphError::StructuralContribution(_)
                ))
            ));
        } else {
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = director.apply(&movement);
                }))
                .is_err()
            );
        }
        assert_eq!(director.assignment().values(), &[Some(0), Some(0)]);
        assert_eq!(director.score(), SimpleScore::ZERO);
        assert_eq!(
            director.graph().source_rows(&inspect).unwrap(),
            &BTreeMap::from([(0, (0, Some(0))), (1, (1, Some(0)))])
        );
        assert!(
            director
                .constraint_scores()
                .all(|(_, s)| s == SimpleScore::ZERO)
        );
    }
    director
        .apply(&Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: Some(1),
            },
            Move::Change {
                variable: 1,
                value: Some(1),
            },
        ]))
        .unwrap()
        .commit();
    assert_eq!(director.score(), SimpleScore::new(4));
}

#[test]
fn binding_rejects_foreign_or_populated_planning_sources_and_bad_assignment_shapes() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 4]).unwrap();
    let (graph, _) = network(Rc::new(Cell::new(0)));
    let (_, foreign) = network(Rc::new(Cell::new(0)));
    assert!(matches!(
        StreamScoreDirector::new(&problem, graph, foreign, problem.initial_assignment()),
        Err(StreamScoringError::Graph(GraphError::ForeignPort))
    ));
    let (mut graph, source) = network(Rc::new(Cell::new(0)));
    let mut tx = graph.transaction();
    tx.apply(&source, Change::put(0, (0, None))).unwrap();
    tx.commit().unwrap();
    assert!(matches!(
        StreamScoreDirector::new(&problem, graph, source, problem.initial_assignment()),
        Err(StreamScoringError::PlanningSourceNotEmpty)
    ));
    let (graph, source) = network(Rc::new(Cell::new(0)));
    assert!(matches!(
        StreamScoreDirector::new(&problem, graph, source, Assignment::new(vec![None; 3])),
        Err(StreamScoringError::Model(
            ModelError::AssignmentLength { .. }
        ))
    ));
}

#[test]
fn incremental_moves_support_exact_enumeration_and_bounded_improvement() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 4]).unwrap();
    let (graph, source) = network(Rc::new(Cell::new(0)));
    let mut director =
        StreamScoreDirector::new(&problem, graph, source, problem.initial_assignment()).unwrap();
    let exact = exhaustive_search(
        &problem,
        |assignment| {
            let movement = Move::Composite(
                assignment
                    .values()
                    .iter()
                    .enumerate()
                    .map(|(variable, &value)| Move::Change { variable, value })
                    .collect(),
            );
            let candidate = director.apply(&movement).unwrap();
            let score = candidate.score();
            assert_eq!(score, total(assignment.values()));
            candidate.commit();
            score
        },
        16,
    );
    assert!(exact.is_optimal());
    assert_eq!(exact.evaluations, 16);
    assert_eq!(exact.best.unwrap().score, HardSoftScore::ZERO);
    director
        .apply(&Move::Composite(
            (0..4)
                .map(|variable| Move::Change {
                    variable,
                    value: Some(0),
                })
                .collect(),
        ))
        .unwrap()
        .commit();
    let mut evaluations = 1;
    let mut steps = 0;
    while evaluations < 200 {
        let base = director.assignment().clone();
        let score = director.score();
        let mut improved = false;
        // Single-variable changes have a strict local optimum at [1,1,0,0].
        // Include swaps so this bounded search can escape that known trap.
        let moves = change_moves(&problem, &base)
            .unwrap()
            .chain(swap_moves(&problem, &base).unwrap());
        for movement in moves {
            if evaluations == 200 {
                break;
            }
            let candidate = director.apply(&movement).unwrap();
            evaluations += 1;
            assert_eq!(candidate.score(), total(candidate.assignment().values()));
            if candidate.score() > score {
                candidate.commit();
                steps += 1;
                improved = true;
                break;
            }
        }
        if !improved {
            break;
        }
    }
    assert!(steps > 0);
    assert!(evaluations <= 200);
    assert_eq!(director.score(), HardSoftScore::ZERO);
    assert_eq!(total(director.assignment().values()), HardSoftScore::ZERO);
}

#[test]
fn replacement_reuses_grouped_graph_for_every_partial_assignment() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 4]).unwrap();
    let calls = Rc::new(Cell::new(0));
    let (g, source) = network(calls.clone());
    let mut d = StreamScoreDirector::new(&p, g, source, p.initial_assignment()).unwrap();
    for state in 0..81 {
        let before = d.assignment().clone();
        let before_score = d.score();
        let mut digits = state;
        let next = Assignment::new(
            (0..4)
                .map(|_| {
                    let v = match digits % 3 {
                        0 => None,
                        1 => Some(0),
                        _ => Some(1),
                    };
                    digits /= 3;
                    v
                })
                .collect(),
        );
        let trial = d.replace_assignment(next.clone()).unwrap();
        assert_eq!(trial.assignment(), &next);
        assert_eq!(trial.score(), total(next.values()));
        assert_eq!(
            trial
                .graph()
                .constraint_scores()
                .collect::<BTreeMap<_, _>>(),
            reference(next.values())
        );
        let work = calls.get();
        if state % 2 == 0 {
            trial.commit();
            assert_eq!(d.assignment(), &next);
        } else {
            drop(trial);
            assert_eq!(d.assignment(), &before);
            assert_eq!(d.score(), before_score);
        }
        assert_eq!(calls.get(), work, "commit/drop must not rescore");
    }
    d.assert_consistent().unwrap();
}
