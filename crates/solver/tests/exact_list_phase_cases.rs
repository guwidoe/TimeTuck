// Adapted Timefold Community TEST, Apache-2.0; see compat/provenance.json.
use timetuck::*;
include!("fixtures/exact_list_trace_fixture.rs");
fn score(a: &ListAssignment<i32>) -> HardSoftScore {
    let hard = a
        .lists()
        .iter()
        .filter(|l| l.len() > 1)
        .map(|l| -(l.len() as i64))
        .sum();
    let soft = -(a.lists().iter().filter(|l| l.is_empty()).count() as i64);
    let remaining = 3 - a.lists().iter().map(Vec::len).sum::<usize>();
    HardSoftScore::new(hard, soft).with_structural(-(remaining as i64))
}
fn notation(a: &ListAssignment<i32>) -> String {
    [1, 0, 2]
        .iter()
        .map(|&i| {
            if a.lists()[i].is_empty() {
                "-".to_string()
            } else {
                format!(
                    "[{}]",
                    a.lists()[i]
                        .iter()
                        .map(i32::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
        })
        .collect()
}
#[test]
fn verify_exhaustive_search_steps() {
    for (index, (bounded, node, entities, values, expected)) in cases().into_iter().enumerate() {
        let p = ListProblem::new(vec![1, 3, 2], vec![ListEntity::new(vec![]); 3])
            .unwrap()
            .allowing_unassigned();
        let entity_order = if entities {
            vec![2, 0, 1]
        } else {
            vec![0, 1, 2]
        };
        let value_order = match values {
            1 => vec![0, 2, 1],
            2 => vec![1, 2, 0],
            _ => vec![0, 1, 2],
        };
        let mut model = ExactListPhaseModel::new(
            &p,
            entity_order,
            value_order,
            score,
            |_, _, s: &HardSoftScore| // Preserve source ONLY_DOWN declaration; callers supply admissible bounds
                // for optimum guarantees, and this declared trend is not inferred.
                Some(s.with_structural(0)),
        )
        .unwrap();
        let config = ExactMovePhaseConfig {
            kind: if bounded {
                ExactPhaseKind::BranchAndBound
            } else {
                ExactPhaseKind::BruteForce
            },
            exploration: node,
            max_steps: 46,
        };
        if let (false, Some(node)) = (bounded, node) {
            assert_eq!(
                config.validate().unwrap_err().to_string(),
                format!(
                    "The phaseConfig (ExhaustiveSearchPhaseConfig) has an nodeExplorationType ({}) which is not compatible with its exhaustiveSearchType (BRUTE_FORCE).",
                    match node {
                        NodeExploration::DepthFirst => "DEPTH_FIRST",
                        NodeExploration::BreadthFirst => "BREADTH_FIRST",
                        NodeExploration::ScoreFirst => "SCORE_FIRST",
                        NodeExploration::OptimisticBoundFirst => "OPTIMISTIC_BOUND_FIRST",
                        _ => unreachable!(),
                    }
                )
            );
            continue;
        }
        let mut steps = Vec::new();
        let result = run_exact_phase(&mut model, config, |a, _| steps.push(notation(a))).unwrap();
        steps.push(notation(&result.best.assignment));
        assert_eq!(steps, expected, "configuration {index}: {config:?}");
        assert!(p.is_complete(&result.best.assignment));
        assert_eq!(result.best.score, HardSoftScore::ZERO);
    }
}
