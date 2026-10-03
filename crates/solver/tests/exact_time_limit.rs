//! Original public elapsed-time controls; no upstream method credit.
use std::time::Duration;
use timetuck::*;
#[test]
fn public_zero_deadlines_prevent_scoring_in_all_exact_models() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap(),
        ListProblem::new(vec![7, 8], vec![ListEntity::new(vec![])]).unwrap(),
    );
    let token = CancellationToken::new();
    token.cancel();
    for budget in [0, 10] {
        for cancelled in [false, true] {
            let c = ExactSearchConfig {
                max_evaluations: budget,
                cancellation: cancelled.then_some(&token),
                max_time: Some(Duration::ZERO),
            };
            let b = exhaustive_search_with_config(
                p.basic(),
                |_| -> SimpleScore { panic!("stopped") },
                c,
            );
            let l = exhaustive_list_search_with_config(
                p.lists(),
                |_| -> SimpleScore { panic!("stopped") },
                c,
            );
            let m = exhaustive_mixed_search_with_config(
                &p,
                |_| -> SimpleScore { panic!("stopped") },
                c,
            );
            let expected = if budget == 0 {
                Termination::EvaluationLimit
            } else if cancelled {
                Termination::Cancelled
            } else {
                Termination::TimeLimit
            };
            for (reason, evaluations, optimal, best) in [
                (
                    b.termination,
                    b.evaluations,
                    b.is_optimal(),
                    b.best.is_some(),
                ),
                (
                    l.termination,
                    l.evaluations,
                    l.is_optimal(),
                    l.best.is_some(),
                ),
                (
                    m.termination,
                    m.evaluations,
                    m.is_optimal(),
                    m.best.is_some(),
                ),
            ] {
                assert_eq!(
                    (reason, evaluations, optimal, best),
                    (expected, 0, false, false)
                );
            }
        }
    }
}
#[test]
fn disabled_and_large_time_limits_preserve_budget_only_results() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).allowing_unassigned()]).unwrap(),
        ListProblem::new(
            vec![7, 8],
            vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
        )
        .unwrap()
        .allowing_unassigned(),
    );
    for budget in 0..=30 {
        for max_time in [None, Some(Duration::MAX)] {
            let c = ExactSearchConfig {
                max_evaluations: budget,
                max_time,
                ..Default::default()
            };
            let score_basic = |a: &Assignment<i32>| {
                SimpleScore::new(a.values().iter().flatten().map(|&v| i64::from(v)).sum())
            };
            let score_list = |a: &ListAssignment<i32>| {
                SimpleScore::new(
                    a.lists()
                        .iter()
                        .enumerate()
                        .map(|(i, list)| {
                            (i as i64 + 1) * list.iter().map(|&v| i64::from(v)).sum::<i64>()
                        })
                        .sum(),
                )
            };
            let score_mixed = |a: &MixedAssignment<i32, i32>| {
                SimpleScore::new(score_basic(a.basic()).score() + score_list(a.lists()).score())
            };
            assert_eq!(
                exhaustive_search_with_config(p.basic(), score_basic, c),
                exhaustive_search(p.basic(), score_basic, budget)
            );
            assert_eq!(
                exhaustive_list_search_with_config(p.lists(), score_list, c),
                exhaustive_list_search(p.lists(), score_list, budget)
            );
            assert_eq!(
                exhaustive_mixed_search_with_config(&p, score_mixed, c),
                exhaustive_mixed_search(&p, score_mixed, budget)
            );
        }
    }
}
