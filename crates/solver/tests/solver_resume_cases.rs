// Adapted from pinned Timefold Community DefaultSolverTest and test fixtures.
// Apache-2.0; provenance and native substitutions: compat/provenance.json.
use timetuck::*;

#[test]
fn solve_repeatedly_basic_variable() {
    let mut problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 5]).unwrap();
    // PlannerTestUtils uses DummySimpleScoreEasyScoreCalculator: ordinary score 0.
    let network = ConstraintNetwork::<_, SimpleScore>::new(5);
    assert_eq!(
        ScoreDirector::new(&problem, &network, problem.initial_assignment())
            .unwrap()
            .score(),
        SimpleScore::ZERO.with_structural(-5)
    );
    for missing in [5, 3, 1] {
        assert_eq!(
            problem
                .initial_assignment()
                .values()
                .iter()
                .filter(|v| v.is_none())
                .count(),
            missing
        );
        let before = problem.initial_assignment();
        let result = solve(
            &problem,
            &network,
            SolverConfig {
                construction: ConstructionConfig {
                    max_steps: Some(2),
                    ..Default::default()
                },
                local_search: vec![],
                ..Default::default()
            },
        )
        .unwrap();
        for (old, new) in before.values().iter().zip(result.assignment.values()) {
            if old.is_some() {
                assert_eq!(old, new);
            }
        }
        problem = problem.with_initial_assignment(&result.assignment).unwrap();
    }
    assert!(
        problem
            .initial_assignment()
            .values()
            .iter()
            .all(Option::is_some)
    );
}

#[test]
fn solve_repeatedly_list_variable() {
    let mut problem = ListProblem::new(
        (0..24).collect::<Vec<_>>(),
        vec![ListEntity::new(vec![]); 8],
    )
    .unwrap();
    let network = ListConstraintNetwork::<_, SimpleScore>::new(8, 24);
    assert_eq!(
        ListScoreDirector::new(&problem, &network, problem.initial_assignment())
            .unwrap()
            .score(),
        SimpleScore::ZERO.with_structural(-24)
    );
    for assigned in [0, 7, 14, 21] {
        let before = problem.initial_assignment();
        assert_eq!(before.lists().iter().map(Vec::len).sum::<usize>(), assigned);
        assert!(assigned < 24);
        let result = solve_list(
            &problem,
            &network,
            SolverConfig {
                construction: ConstructionConfig {
                    max_steps: Some(7),
                    ..Default::default()
                },
                local_search: vec![],
                ..Default::default()
            },
        )
        .unwrap();
        for value in 0..24 {
            if let Some(old) = before.location(&value) {
                assert_eq!(
                    result.assignment.location(&value).unwrap().entity,
                    old.entity
                );
            }
        }
        problem = problem.with_initial_assignment(&result.assignment).unwrap();
    }
    assert_eq!(
        problem
            .initial_assignment()
            .lists()
            .iter()
            .map(Vec::len)
            .sum::<usize>(),
        24
    );
    assert!(problem.is_complete(&problem.initial_assignment()));
}

fn mixed_problem(
    entities: usize,
    values: usize,
    optional: bool,
    pin: bool,
    prefix: bool,
) -> MixedProblem<usize, usize> {
    let mut basics = Vec::new();
    let mut lists = Vec::new();
    for e in 0..entities {
        for field in 0..2 {
            let mut variable = Variable::new(vec![0, 1]);
            if optional && field == 0 {
                variable = variable.allowing_unassigned();
            }
            if e == 0 && pin {
                if optional {
                    variable = variable.with_initial(0);
                }
                variable = variable.with_pinned(true);
            }
            basics.push(variable);
        }
        lists.push(if e == 0 && pin {
            ListEntity::new(if optional { vec![0] } else { vec![] }).with_pinned(true)
        } else if e == 0 && prefix {
            ListEntity::new(vec![1, 2]).with_pinned_prefix(2)
        } else {
            ListEntity::new(vec![])
        });
    }
    let mut list_problem = ListProblem::new((0..values).collect(), lists).unwrap();
    if optional {
        list_problem = list_problem.allowing_unassigned();
    }
    MixedProblem::new(FiniteProblem::new(basics).unwrap(), list_problem)
}

fn mixed_network(
    entities: usize,
    values: usize,
    optional: bool,
    blocked_lists: bool,
) -> MixedConstraintNetwork<'static, usize, usize, SimpleScore> {
    let mut network = MixedConstraintNetwork::new(2 * entities, entities, values);
    for e in 0..entities {
        network
            .add_constraint(
                format!("entity {e}"),
                vec![
                    MixedScope::Basic(2 * e),
                    MixedScope::Basic(2 * e + 1),
                    MixedScope::ListEntity(e),
                ],
                move |facts| {
                    let MixedFact::Basic(first) = facts[0] else {
                        panic!("basic")
                    };
                    let MixedFact::Basic(second) = facts[1] else {
                        panic!("second basic")
                    };
                    let MixedFact::ListEntity(list) = facts[2] else {
                        panic!("list")
                    };
                    // Exact test calculators: optional fixtures block every basic value;
                    // list blocking varies between the two pinned/unassigned scenarios.
                    let mut score = if first.is_some() {
                        if optional { -10 } else { 1 }
                    } else {
                        0
                    };
                    score += i64::from(second.is_some());
                    score += if optional && blocked_lists && !list.is_empty() {
                        -10
                    } else if list.len() == if optional { 3 } else { 1 } {
                        2
                    } else {
                        1
                    };
                    SimpleScore::new(score)
                },
            )
            .unwrap();
    }
    network
}

fn mixed_solve(
    problem: &MixedProblem<usize, usize>,
    optional: bool,
    blocked_lists: bool,
    local: bool,
) -> SolverResult<MixedAssignment<usize, usize>, SimpleScore> {
    let network = mixed_network(
        problem.lists().entities().len(),
        problem.lists().values().len(),
        optional,
        blocked_lists,
    );
    let mut events = 0;
    let result = solve_mixed_with_observer(
        problem,
        &network,
        MixedSolverConfig {
            construction: MixedConstructionConfig {
                construction: ConstructionConfig {
                    optional: OptionalConstruction::NonWorsening,
                    ..Default::default()
                },
                ..Default::default()
            },
            local_search: if local {
                vec![MixedLocalSearchConfig {
                    search: LocalSearchConfig {
                        max_steps: 16,
                        move_order: MoveOrder::Original,
                        ..Default::default()
                    },
                    ..Default::default()
                }]
            } else {
                vec![]
            },
            ..Default::default()
        },
        |_| events += 1,
    )
    .unwrap();
    assert!(events > 0); // PlannerTestUtils.solve also requires a best-score event.
    result
}

#[test]
fn solve_pinned_mixed_model() {
    let problem = mixed_problem(3, 2, false, true, false);
    let result = mixed_solve(&problem, false, false, false);
    assert_eq!(&result.assignment.basic().values()[..2], &[None, None]);
    assert!(result.assignment.lists().lists()[0].is_empty());
    // Additional native evidence: movable fields initialize, null pins stay structural.
    assert!(
        result.assignment.basic().values()[2..]
            .iter()
            .all(Option::is_some)
    );
    assert_eq!(
        result
            .assignment
            .lists()
            .lists()
            .iter()
            .map(Vec::len)
            .sum::<usize>(),
        2
    );
    assert!(!result.complete);
    assert!(result.solution().is_none());
    assert_eq!(result.score.unwrap().structural(), -2);
}

#[test]
fn solve_unassigned_mixed_model() {
    let problem = mixed_problem(2, 2, true, false, false);
    let result = mixed_solve(&problem, true, true, true);
    for entity in 0..2 {
        assert!(result.assignment.basic().values()[2 * entity].is_none());
        assert!(result.assignment.basic().values()[2 * entity + 1].is_some());
        assert!(result.assignment.lists().lists()[entity].is_empty());
    }
    assert!(result.complete);
}

#[test]
fn solve_pinned_and_unassigned_mixed_model() {
    let problem = mixed_problem(2, 2, true, true, false);
    let result = mixed_solve(&problem, true, true, true);
    assert_eq!(
        &result.assignment.basic().values()[..2],
        &[Some(0), Some(0)]
    );
    assert_eq!(result.assignment.lists().lists()[0], [0]);
    assert!(result.assignment.basic().values()[2].is_none());
    assert!(result.assignment.basic().values()[3].is_some());
    assert!(result.assignment.lists().lists()[1].is_empty());

    let problem = mixed_problem(2, 4, true, false, true);
    let result = mixed_solve(&problem, true, false, true);
    for entity in 0..2 {
        assert!(result.assignment.basic().values()[2 * entity].is_none());
        assert!(result.assignment.basic().values()[2 * entity + 1].is_some());
    }
    let first = &result.assignment.lists().lists()[0];
    assert_eq!(first.len(), 3);
    assert_eq!(&first[..2], &[1, 2]);
    let mut unordered = first.clone();
    unordered.sort();
    assert_eq!(unordered, [0, 1, 2]);
    assert_eq!(result.assignment.lists().lists()[1], [3]);
}
