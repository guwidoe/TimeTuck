//! Original frontier-order contracts; no upstream method credit or Java step parity.
use std::cmp::Ordering;
use timetuck::*;

fn config(order: NodeExploration) -> BranchAndBoundConfig<'static> {
    BranchAndBoundConfig {
        exploration: order,
        ..Default::default()
    }
}

#[test]
fn scheduling_ignores_score_bound_and_parent_metadata() {
    let order = NodeExploration::InputOrderDepthFirst;
    let first = SearchNodeKey {
        depth: 2,
        score: SimpleScore::new(-100),
        optimistic_bound: Some(SimpleScore::new(-100)),
        parent_breadth: 0,
        breadth: 0,
    };
    let later = SearchNodeKey {
        score: SimpleScore::new(100),
        optimistic_bound: None,
        parent_breadth: 999,
        breadth: 1,
        ..first.clone()
    };
    assert_eq!(order.compare(&first, &later), Ordering::Greater);
    assert_eq!(order.compare(&later, &first), Ordering::Less);
    let same_choice = SearchNodeKey {
        breadth: 0,
        ..later.clone()
    };
    assert_eq!(order.compare(&first, &same_choice), Ordering::Equal);
    let deeper = SearchNodeKey {
        depth: 3,
        ..later.clone()
    };
    assert_eq!(order.compare(&deeper, &first), Ordering::Greater);
    assert_eq!(
        NodeExploration::DepthFirst.compare(&first, &later),
        Ordering::Less
    );
}

#[test]
fn generation_scores_siblings_before_depth_first_expansion_and_retains_first_tie() {
    let p = FiniteProblem::new(vec![Variable::new(vec![9, 2]); 3]).unwrap();
    let expected = vec![
        vec![None, None, None],
        vec![Some(9), None, None],
        vec![Some(2), None, None],
        vec![Some(9), Some(9), None],
        vec![Some(9), Some(2), None],
        vec![Some(9), Some(9), Some(9)],
        vec![Some(9), Some(9), Some(2)],
        vec![Some(9), Some(2), Some(9)],
        vec![Some(9), Some(2), Some(2)],
        vec![Some(2), Some(9), None],
        vec![Some(2), Some(2), None],
        vec![Some(2), Some(9), Some(9)],
        vec![Some(2), Some(9), Some(2)],
        vec![Some(2), Some(2), Some(9)],
        vec![Some(2), Some(2), Some(2)],
    ];
    // Every partial score misleadingly favors later choices. Complete ties remain
    // genuine ties; a scheduling estimate must not become a pruning bound.
    let score = |a: &Assignment<i32>, remaining: &[usize]| {
        SimpleScore::new(if remaining.is_empty() {
            0
        } else {
            -a.values()
                .iter()
                .flatten()
                .map(|v| i64::from(*v))
                .sum::<i64>()
        })
    };
    for budget in 0..=expected.len() {
        let mut trace = Vec::new();
        let result = branch_and_bound(
            &p,
            |a, remaining| {
                trace.push(a.values().to_vec());
                BranchEvaluation::unbounded(score(a, remaining))
            },
            BranchAndBoundConfig {
                max_evaluations: budget as u64,
                ..config(NodeExploration::InputOrderDepthFirst)
            },
        )
        .unwrap();
        assert_eq!(trace, expected[..budget]);
        assert_eq!(result.evaluations, budget as u64);
        assert_eq!(
            result.complete_evaluations,
            expected[..budget]
                .iter()
                .filter(|v| v.iter().all(Option::is_some))
                .count() as u64
        );
        assert_eq!(
            result.best.as_ref().map(|b| b.assignment.values()),
            (budget >= 6).then_some(expected[5].as_slice())
        );
        assert_eq!(result.is_optimal(), budget == expected.len());
    }
    let guided = branch_and_bound(
        &p,
        |a, remaining| BranchEvaluation::unbounded(score(a, remaining)),
        config(NodeExploration::DepthFirst),
    )
    .unwrap();
    assert_eq!(
        guided.best.unwrap().assignment.values(),
        &[Some(2), Some(2), Some(9)]
    );
}

#[test]
fn list_input_order_means_entity_then_insertion_index_and_mixed_lists_come_first() {
    let lists = ListProblem::new(
        vec![7, 2],
        vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
    )
    .unwrap();
    let expected = vec![
        vec![vec![], vec![]],
        vec![vec![7], vec![]],
        vec![vec![], vec![7]],
        vec![vec![2, 7], vec![]],
        vec![vec![7, 2], vec![]],
        vec![vec![7], vec![2]],
        vec![vec![2], vec![7]],
        vec![vec![], vec![2, 7]],
        vec![vec![], vec![7, 2]],
    ];
    let mut trace = Vec::new();
    let result = branch_and_bound_lists(
        &lists,
        |a, remaining| {
            trace.push(a.lists().to_vec());
            BranchEvaluation::unbounded(SimpleScore::new(if remaining.is_empty() {
                0
            } else {
                a.lists()[1].len() as i64
            }))
        },
        config(NodeExploration::InputOrderDepthFirst),
    )
    .unwrap();
    assert_eq!(trace, expected);
    assert_eq!(result.best.unwrap().assignment.lists(), &expected[3]);
    assert_eq!(result.complete_evaluations, 6);

    let mixed = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![9, 2]); 2]).unwrap(),
        lists,
    );
    let mut leaves = Vec::new();
    let result = branch_and_bound_mixed(
        &mixed,
        |a, basic_remaining, list_remaining| {
            let complete = basic_remaining.is_empty() && list_remaining.is_empty();
            if complete {
                leaves.push(a.clone());
            }
            BranchEvaluation::unbounded(SimpleScore::new(if complete {
                0
            } else {
                a.lists().lists()[1].len() as i64 * 100
                    - a.basic()
                        .values()
                        .iter()
                        .flatten()
                        .map(|v| i64::from(*v))
                        .sum::<i64>()
            }))
        },
        config(NodeExploration::InputOrderDepthFirst),
    )
    .unwrap();
    // Independent Cartesian enumeration: the six two-value owner/order states
    // above, each paired with four domain assignments in declared order.
    let mut reference = Vec::new();
    for rows in &expected[3..] {
        for x in [9, 2] {
            for y in [9, 2] {
                reference.push(MixedAssignment::new(
                    Assignment::new(vec![Some(x), Some(y)]),
                    ListAssignment::new(rows.clone()),
                ));
            }
        }
    }
    assert_eq!(leaves, reference);
    assert_eq!(result.best.unwrap().assignment, reference[0]);
    assert_eq!(result.complete_evaluations, 24);
}
