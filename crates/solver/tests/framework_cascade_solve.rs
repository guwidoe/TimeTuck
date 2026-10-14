// Adapted native behavioral qualification of pinned Community
// TimefoldProcessorDuplicateCascadingShadowVariableSolveTest#solve and its
// TestdataQuarkusDuplicateCascading TEST fixtures. Apache-2.0;
// see THIRD_PARTY_NOTICES.md and compat/licenses/timefold-Apache-2.0.txt.
// Quarkus injection, archive generation and duplicate GeneratedClassBuildItem
// detection remain outside this native behavior qualification; no DI/build parity.
use std::{cell::RefCell, rc::Rc, time::Duration};
use timetuck::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Calculation {
    chain_length: i32,
    chain_product: i32,
}

// Both fields are one callback result, as in updateCalculationValue(); there
// are no independently registered callbacks for the two cascading fields.
fn update_calculation_value(previous: Option<&Calculation>) -> Calculation {
    previous.map_or(
        Calculation {
            chain_length: 0,
            chain_product: 1,
        },
        |previous| Calculation {
            chain_length: previous.chain_length + 1,
            chain_product: previous.chain_product * 2,
        },
    )
}

fn problem(warm_start: bool) -> ListProblem<String> {
    let values: Vec<_> = (1..5).map(|i| format!("v{i}")).collect();
    let entities = if warm_start {
        vec![
            ListEntity::new(values[..2].to_vec()),
            ListEntity::new(values[2..].to_vec()),
        ]
    } else {
        vec![ListEntity::new(vec![]); 2]
    };
    ListProblem::new(values, entities).unwrap()
}

fn config() -> SolverConfig<'static> {
    SolverConfig {
        limits: SolverLimits {
            max_evaluations: 512,
            max_move_evaluations: Some(256),
            max_time: Some(Duration::from_secs(5)),
            best_score_limit: Some(SimpleScore::new(-3)),
            assert_phase_scores: true,
            ..Default::default()
        },
        construction: ConstructionConfig {
            max_evaluations: 128,
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            max_evaluations: 256,
            max_steps: 16,
            selection: MoveSelection::BestAccepted,
            assert_scores: true,
            ..Default::default()
        }],
    }
}

fn network(p: &ListProblem<String>) -> ListConstraintNetwork<'static, String, SimpleScore> {
    let cache = Rc::new(RefCell::new(
        ListCascadeCache::new(p, &p.initial_assignment(), |c| {
            c.position
                .map(|_| update_calculation_value(c.previous_shadow))
        })
        .unwrap(),
    ));
    let mut network = ListConstraintNetwork::new(2, 4);
    for (name, length_constraint) in [("length too long", true), ("product too small", false)] {
        let cache = cache.clone();
        network
            .add_constraint(
                name,
                vec![ListScope::Entity(0), ListScope::Entity(1)],
                move |facts| {
                    let assignment = ListAssignment::new(
                        facts
                            .iter()
                            .map(|fact| {
                                let ListFact::Entity(row) = fact else {
                                    panic!("entity scope required")
                                };
                                row.to_vec()
                            })
                            .collect(),
                    );
                    let mut cache = cache.borrow_mut();
                    cache
                        .update(&assignment, &[], |c| {
                            c.position
                                .map(|_| update_calculation_value(c.previous_shadow))
                        })
                        .unwrap();
                    SimpleScore::new(
                        -(cache
                            .shadows()
                            .values()
                            .flatten()
                            .filter(|calculation| {
                                if length_constraint {
                                    calculation.chain_length > 2
                                } else {
                                    calculation.chain_product < 4
                                }
                            })
                            .count() as i64),
                    )
                },
            )
            .unwrap();
    }
    network
}

type Matches = StreamConstraint<usize, (ListValueRow<String>, DerivedValue<Calculation>)>;
type ScoringGraph = (
    StreamGraph<'static, SimpleScore>,
    ListStreamSources<String>,
    [Matches; 2],
);
fn graph(p: &ListProblem<String>) -> Result<ScoringGraph, GraphError> {
    let mut builder = StreamGraphBuilder::new();
    let sources = ListStreamSources::new(&mut builder);
    let cascade = list_cascade_stream(&mut builder, &sources, p, |c| {
        Some(update_calculation_value(c.previous_shadow))
    })?;
    let length = builder.score("length too long", &cascade, |(_, shadow)| {
        SimpleScore::new(-i64::from(shadow.value.is_some_and(|c| c.chain_length > 2)))
    })?;
    let product = builder.score("product too small", &cascade, |(_, shadow)| {
        SimpleScore::new(-i64::from(
            shadow.value.is_some_and(|c| c.chain_product < 4),
        ))
    })?;
    Ok((builder.build(), sources, [length, product]))
}

// Independent positional formulas avoid using the cascade callback as oracle.
fn reference_score(assignment: &ListAssignment<String>) -> SimpleScore {
    SimpleScore::new(
        -(assignment
            .lists()
            .iter()
            .map(|row| row.len().saturating_sub(3) + row.len().min(2))
            .sum::<usize>() as i64),
    )
}

fn check_solution(
    p: &ListProblem<String>,
    solution: &SolverResult<ListAssignment<String>, SimpleScore>,
) {
    assert!(solution.complete);
    p.validate(&solution.assignment).unwrap();
    assert!(p.is_complete(&solution.assignment));
    let mut values: Vec<_> = solution
        .assignment
        .lists()
        .iter()
        .flatten()
        .cloned()
        .collect();
    values.sort();
    assert_eq!(values, p.values());
    assert_eq!(solution.score, Some(SimpleScore::new(-3)));
    assert_eq!(solution.score, Some(reference_score(&solution.assignment)));
    assert_eq!(solution.termination, SolverTermination::BestScoreLimit);
    assert!(solution.work.evaluations <= 512);
    assert!(solution.work.move_evaluations <= 256);
    assert!(
        !solution.is_optimal(),
        "bounded heuristic is not an exact proof"
    );

    let cache = ListCascadeCache::new(p, &solution.assignment, |c| {
        c.position
            .map(|_| update_calculation_value(c.previous_shadow))
    })
    .unwrap();
    let (g, sources, matches) = graph(p).unwrap();
    let mut director =
        ListStreamScoreDirector::new(p, g, sources, solution.assignment.clone()).unwrap();
    assert_eq!(director.score(), reference_score(&solution.assignment));
    director.assert_consistent().unwrap();
    for match_set in matches {
        let rows = director.graph().constraint_matches(&match_set).unwrap();
        assert_eq!(rows.len(), 4);
        for (registry, (row_shadow, _)) in rows.iter() {
            let (row, shadow) = row_shadow;
            assert_eq!(row.value, p.values()[*registry]);
            let element = solution.assignment.element(&row.value).unwrap();
            assert_eq!(row.position, Some(element.position));
            assert_eq!(row.previous.as_ref(), element.previous);
            assert_eq!(row.next.as_ref(), element.next);
            let expected = Calculation {
                chain_length: element.position.index as i32,
                chain_product: 1 << element.position.index,
            };
            assert_eq!(cache.shadows()[&row.value], Some(expected));
            assert_eq!(shadow.value, Some(expected));
            assert!(!shadow.inconsistent);
        }
    }
}

fn solve_managed(warm_start: bool) -> [SolverResult<ListAssignment<String>, SimpleScore>; 2] {
    let mut manager =
        SolverManager::<SolverResult<ListAssignment<String>, SimpleScore>, String>::new(1).unwrap();
    let mut solutions = vec![];
    for stream in [false, true] {
        let job = manager
            .submit(1, move |_| {
                let p = problem(warm_start);
                let result = if stream {
                    solve_list_stream(
                        &p,
                        || {
                            let (graph, sources, _) = graph(&p)?;
                            Ok((graph, sources))
                        },
                        config(),
                    )
                } else {
                    solve_list(&p, &network(&p), config())
                };
                result.map_err(|error| error.to_string())
            })
            .unwrap();
        // Native successful final-result unwrap preserves assertNotNull(solution).
        let final_result = job.wait_timeout(Duration::from_secs(10)).unwrap();
        let solution = final_result.as_ref().as_ref().unwrap();
        check_solution(&problem(warm_start), solution);
        solutions.push(solution.clone());
    }
    manager.shutdown().unwrap();
    let solutions: [_; 2] = solutions.try_into().unwrap();
    assert_eq!(solutions[0].assignment, solutions[1].assignment);
    assert_eq!(solutions[0].score, solutions[1].score);
    solutions
}

#[test]
fn duplicate_cascading_shadow_variable_solve_native_qualification() {
    // Exact source fixture: v1..v4 and two initially empty entities, target -3.
    solve_managed(false);
}

#[test]
fn grouped_fields_guide_real_improvement_from_balanced_warm_start() {
    // Original control: -4 warm start forces a real local-search improvement;
    // the returned assignment is never prescribed to either solver backend.
    assert_eq!(
        reference_score(&problem(true).initial_assignment()),
        SimpleScore::new(-4)
    );
    let solutions = solve_managed(true);
    for solution in solutions {
        assert!(solution.work.move_evaluations > 0);
        assert!(solution.work.accepted_moves > 0);
    }
}
