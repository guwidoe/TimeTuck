// Complete pinned Community black-box basic exhaustive TEST adaptation.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/exact_basic_trace_rows.rs"]
mod fixture;
use timetuck::*;
fn render(a: &Assignment<i32>) -> String {
    a.values()
        .iter()
        .map(|v| v.map_or('-', |v| char::from_digit(v as u32, 10).unwrap()))
        .collect()
}
#[test]
fn verify_exhaustive_search_steps() {
    let rows = fixture::rows();
    assert_eq!(rows.len(), 40);
    for (kind, exploration, sort_entities, sort_values, expected) in rows {
        let config = ExactMovePhaseConfig {
            kind,
            exploration,
            max_steps: 10,
        };
        if expected.is_none() {
            let error = config.validate().unwrap_err();
            let label = match exploration.unwrap() {
                NodeExploration::DepthFirst => "DEPTH_FIRST",
                NodeExploration::BreadthFirst => "BREADTH_FIRST",
                NodeExploration::ScoreFirst => "SCORE_FIRST",
                NodeExploration::OptimisticBoundFirst => "OPTIMISTIC_BOUND_FIRST",
                _ => panic!(),
            };
            assert_eq!(
                error.to_string(),
                format!(
                    "The phaseConfig (ExhaustiveSearchPhaseConfig) has an nodeExplorationType ({label}) which is not compatible with its exhaustiveSearchType (BRUTE_FORCE)."
                )
            );
            continue;
        }
        let p = FiniteProblem::new(vec![Variable::new(vec![1, 3, 2]); 4]).unwrap();
        let mut n = ConstraintNetwork::new(4);
        n.add_constraint("duplicate", vec![0, 1, 2, 3], |values| {
            let mut seen = vec![];
            let mut score = 0;
            for value in values.iter().flatten() {
                if seen.contains(value) {
                    score -= 1;
                } else {
                    seen.push(*value);
                }
            }
            SimpleScore::new(score)
        })
        .unwrap();
        let entity_codes = ["entity4", "entity2", "entity3", "entity1"];
        let mut entities: Vec<_> = (0..entity_codes.len()).collect();
        if sort_entities {
            entities.sort_by(|&a, &b| entity_codes[b].cmp(entity_codes[a]));
        }
        let domain = p.variables()[0].domain();
        let mut values: Vec<_> = (0..domain.len()).collect();
        match sort_values {
            "NONE" => {}
            "ASCENDING" => values.sort_by_key(|&i| domain[i]),
            "DESCENDING" => values.sort_by_key(|&i| std::cmp::Reverse(domain[i])),
            _ => panic!(),
        }
        let mut model = ExactBasicPhaseModel::new(
            &p,
            &n,
            entities,
            vec![values; 4],
            |_: &Assignment<i32>, s: &SimpleScore| Some((*s).with_structural(0)),
        )
        .unwrap();
        let mut trace = vec![];
        let result = run_exact_phase(&mut model, config, |a, _| trace.push(render(a))).unwrap();
        trace.push(render(&result.best.assignment));
        assert_eq!(
            trace,
            expected.unwrap(),
            "{kind:?}/{exploration:?}/{sort_entities}/{sort_values}"
        );
        assert_eq!(result.steps, 10);
    }
}
