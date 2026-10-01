// Original search regression tests. These do not count as upstream adaptations.
use std::collections::HashSet;
use timetuck::{
    Assignment, FiniteProblem, HardSoftScore, SimpleScore, Termination, Variable, exhaustive_search,
};

fn simple_problem(count: usize, values: i32) -> FiniteProblem<i32> {
    FiniteProblem::new(
        (0..count)
            .map(|_| Variable::new((0..values).collect()))
            .collect(),
    )
    .unwrap()
}

#[test]
fn four_queens_enumerates_every_assignment_and_finds_both_optima() {
    let problem = simple_problem(4, 4);
    let original = problem.initial_assignment();
    let mut visited = HashSet::new();
    let mut optima = Vec::new();
    let result = exhaustive_search(
        &problem,
        |state| {
            assert!(problem.is_complete(state));
            assert!(
                visited.insert(state.values().to_vec()),
                "duplicate assignment"
            );
            let mut conflicts = 0;
            for (column, row) in state.values().iter().enumerate() {
                for (other_column, other_row) in state.values().iter().enumerate().skip(column + 1)
                {
                    if row == other_row
                        || (row.unwrap() - other_row.unwrap()).unsigned_abs() as usize
                            == other_column - column
                    {
                        conflicts += 1;
                    }
                }
            }
            if conflicts == 0 {
                optima.push(state.clone());
            }
            SimpleScore::new(-conflicts)
        },
        256,
    );
    assert_eq!(result.evaluations, 256);
    assert_eq!(visited.len(), 256);
    assert!(result.is_optimal());
    assert_eq!(result.best.as_ref().unwrap().score, SimpleScore::ZERO);
    assert_eq!(
        optima,
        vec![
            Assignment::new(vec![Some(1), Some(3), Some(0), Some(2)]),
            Assignment::new(vec![Some(2), Some(0), Some(3), Some(1)])
        ]
    );
    assert_eq!(result.best.unwrap().assignment, optima[0]);
    assert_eq!(problem.initial_assignment(), original);
}

#[test]
fn matches_independent_bitmask_oracle_for_generated_binary_problems() {
    // Numeric masks enumerate assignments independently of the solver's domain
    // odometer. Hard penalties and signed soft preferences vary across fixtures.
    for seed in 0..32_i64 {
        let score = |bits: &[i32]| {
            let hard = -((0..5)
                .filter(|&i| bits[i] == bits[(i + 1) % 5] && (seed + i as i64) % 3 != 0)
                .count() as i64);
            let soft = bits
                .iter()
                .enumerate()
                .map(|(i, &v)| i64::from(v) * ((seed * 7 + i as i64 * 3) % 13 - 6))
                .sum();
            HardSoftScore::new(hard, soft)
        };
        let expected = (0..32)
            .map(|mask| {
                let bits: Vec<i32> = (0..5).map(|i| (mask >> i) & 1).collect();
                score(&bits)
            })
            .max()
            .unwrap();
        let problem = simple_problem(5, 2);
        let result = exhaustive_search(
            &problem,
            |state| {
                score(
                    &state
                        .values()
                        .iter()
                        .map(|v| v.unwrap())
                        .collect::<Vec<_>>(),
                )
            },
            32,
        );
        assert_eq!(result.best.as_ref().unwrap().score, expected);
        let best = result.best.unwrap();
        assert_eq!(
            best.score,
            score(
                &best
                    .assignment
                    .values()
                    .iter()
                    .map(|v| v.unwrap())
                    .collect::<Vec<_>>()
            )
        );
        assert_eq!(result.termination, Termination::Exhausted);
    }
}

#[test]
fn evaluation_budgets_preserve_truthful_termination() {
    let problem = simple_problem(2, 2);
    for budget in 0..=6 {
        let mut calls = 0;
        let result = exhaustive_search(
            &problem,
            |_| {
                calls += 1;
                SimpleScore::ZERO
            },
            budget,
        );
        assert_eq!(result.evaluations, budget.min(4));
        assert_eq!(calls, result.evaluations);
        assert_eq!(result.best.is_some(), budget > 0);
        assert_eq!(
            result.termination,
            if budget < 4 {
                Termination::EvaluationLimit
            } else {
                Termination::Exhausted
            }
        );
        assert_eq!(result.is_optimal(), budget >= 4);
    }
}

#[test]
fn pins_optional_values_and_initial_values_have_distinct_meanings() {
    let problem = FiniteProblem::new(vec![
        Variable::new(vec![9, 8]).with_initial(8).with_pinned(true),
        Variable::new(vec![2, 1])
            .with_initial(1)
            .allowing_unassigned(),
        Variable::new(vec![7])
            .allowing_unassigned()
            .with_pinned(true),
        Variable::new(vec![5, 4]).with_initial(4),
    ])
    .unwrap();
    assert_eq!(problem.search_space_size(), Some(6));
    let mut visited = Vec::new();
    let result = exhaustive_search(
        &problem,
        |state| {
            assert!(problem.is_complete(state));
            visited.push(state.values().to_vec());
            SimpleScore::ZERO
        },
        6,
    );
    assert_eq!(
        visited,
        vec![
            vec![Some(8), Some(2), None, Some(5)],
            vec![Some(8), Some(2), None, Some(4)],
            vec![Some(8), Some(1), None, Some(5)],
            vec![Some(8), Some(1), None, Some(4)],
            vec![Some(8), None, None, Some(5)],
            vec![Some(8), None, None, Some(4)],
        ]
    );
    assert_eq!(result.best.unwrap().assignment.values(), visited[0]);
    assert_eq!(result.termination, Termination::Exhausted);
}

#[test]
fn empty_model_has_one_assignment_but_empty_required_domain_has_none() {
    let empty = simple_problem(0, 0);
    assert_eq!(empty.search_space_size(), Some(1));
    let result = exhaustive_search(
        &empty,
        |state| {
            assert!(state.values().is_empty());
            SimpleScore::ZERO
        },
        1,
    );
    assert_eq!(result.evaluations, 1);
    assert!(result.is_optimal());
    let impossible = simple_problem(1, 0);
    for budget in [0, 1, u64::MAX] {
        let result = exhaustive_search(
            &impossible,
            |_| -> SimpleScore { panic!("must not score empty space") },
            budget,
        );
        assert_eq!(result.evaluations, 0);
        assert_eq!(result.termination, Termination::Exhausted);
        assert!(result.best.is_none());
        assert!(!result.is_optimal());
    }
    let optional =
        FiniteProblem::new(vec![Variable::<i32>::new(vec![]).allowing_unassigned()]).unwrap();
    let result = exhaustive_search(
        &optional,
        |state| {
            assert_eq!(state.values(), &[None]);
            SimpleScore::ZERO
        },
        1,
    );
    assert!(result.is_optimal());
}

#[test]
fn structural_invalidity_is_rejected_while_hard_infeasibility_is_retained() {
    let problem = simple_problem(1, 3);
    let result = exhaustive_search(
        &problem,
        |state| match state.values()[0] {
            Some(0) => HardSoftScore::new(1_000, 1_000).with_structural(-1),
            Some(1) => HardSoftScore::new(-2, 999),
            _ => HardSoftScore::new(-1, -999),
        },
        3,
    );
    assert!(result.is_optimal());
    let best = result.best.unwrap();
    assert!(!best.score.is_feasible());
    assert_eq!(best.assignment.values(), &[Some(2)]);
    let invalid = exhaustive_search(&problem, |_| SimpleScore::new(999).with_structural(-1), 3);
    assert_eq!(invalid.termination, Termination::Exhausted);
    assert!(invalid.best.is_none());
    assert!(!invalid.is_optimal());
}

#[test]
fn overflowing_space_count_can_still_be_searched_with_a_small_budget() {
    let problem = simple_problem(130, 2);
    assert_eq!(problem.search_space_size(), None);
    let result = exhaustive_search(&problem, |_| SimpleScore::ZERO, 2);
    assert_eq!(result.evaluations, 2);
    assert_eq!(result.termination, Termination::EvaluationLimit);
    let mut variables = problem.variables().to_vec();
    variables.push(Variable::new(vec![]));
    let impossible = FiniteProblem::new(variables).unwrap();
    assert_eq!(impossible.search_space_size(), Some(0));
}

#[test]
fn many_singleton_variables_do_not_require_recursive_enumeration() {
    let problem = simple_problem(20_000, 1);
    assert_eq!(problem.search_space_size(), Some(1));
    let result = exhaustive_search(&problem, |_| SimpleScore::ZERO, 1);
    assert!(result.is_optimal());
    assert_eq!(result.evaluations, 1);
}
