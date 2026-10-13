// Complete pinned Community mixed exhaustive TEST adaptation; Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::cell::RefCell;
use timetuck::*;
fn render(a: &MixedAssignment<i32, i32>) -> String {
    let mut text = String::new();
    for row in a.basic().values().chunks(2) {
        text.push('(');
        for v in row {
            text.push(v.map_or('-', |v| char::from_digit(v as u32, 10).unwrap()));
        }
        text.push(')');
    }
    for row in a.lists().lists() {
        text.push('[');
        text.push_str(&if row.is_empty() {
            "-".into()
        } else {
            row.iter().map(i32::to_string).collect::<Vec<_>>().join(",")
        });
        text.push(']');
    }
    text
}
#[test]
fn verify_exhaustive_search_steps() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../compat/oracle/fixtures/mixed_exact_trace_observations.json"
    ))
    .unwrap();
    let rows = fixture["observations"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        let kind = if row["kind"] == "BRUTE_FORCE" {
            ExactPhaseKind::BruteForce
        } else {
            ExactPhaseKind::BranchAndBound
        };
        let exploration =
            (kind == ExactPhaseKind::BranchAndBound).then_some(NodeExploration::DepthFirst);
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![1, 2, 3]); 4]).unwrap(),
            ListProblem::new(vec![4, 5, 6], vec![ListEntity::new(vec![]); 2]).unwrap(),
        );
        let mut network = MixedConstraintNetwork::new(4, 2, 3);
        network
            .add_constraint(
                "mixed TEST objective",
                vec![
                    MixedScope::Basic(0),
                    MixedScope::Basic(1),
                    MixedScope::Basic(2),
                    MixedScope::Basic(3),
                    MixedScope::ListEntity(0),
                    MixedScope::ListEntity(1),
                ],
                |facts| {
                    let mut basic = vec![];
                    let mut score = 0;
                    for fact in facts {
                        match fact {
                            MixedFact::Basic(v) => basic.push(v.copied()),
                            MixedFact::ListEntity(list) => {
                                if list.is_empty() {
                                    score -= 1;
                                } else if list.len() > 1 {
                                    score -= 5;
                                }
                            }
                            _ => panic!("scope"),
                        }
                    }
                    let mut used = vec![];
                    for group in basic.chunks(2) {
                        if group[0] == group[1] {
                            score -= 1;
                        }
                        if let Some(primary) = group[0] {
                            if used.contains(&primary) {
                                score -= 5;
                            }
                            used.push(primary);
                        }
                    }
                    SimpleScore::new(score)
                },
            )
            .unwrap();
        let plan = ExactMixedPhasePlan {
            basic_groups: vec![vec![0, 1], vec![2, 3]],
            entity_order: vec![0, 1],
            value_order: vec![0, 1, 2],
            list_roots: 2,
        };
        let trace = RefCell::new(vec![]);
        let result = run_exact_mixed_phase(
            &p,
            &plan,
            ExactMovePhaseConfig {
                kind,
                exploration,
                max_steps: 37,
            },
            |a| {
                trace.borrow_mut().push(render(a));
                network.calculate(&p, a)
            },
            |_, s: &SimpleScore| Some((*s).with_structural(0)),
        )
        .unwrap();
        let expected: Vec<_> = row["calculator_trace"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_owned())
            .collect();
        let actual = trace.into_inner();
        assert_eq!(actual, expected, "{kind:?}");
        assert_eq!(result.basic_steps + result.list_steps, 37);
        assert_eq!(
            render(&result.best.assignment),
            row["returned_best"].as_str().unwrap()
        );
        assert_eq!(result.best.score, SimpleScore::new(-5));
    }
}
