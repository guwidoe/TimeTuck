// Adapted from pinned Timefold Community AbstractUniConstraintStreamTest.
// Apache-2.0; source identities, fixtures and substitutions: compat/provenance.json.
use std::{fmt::Debug, str::FromStr};
use timetuck::*;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Entity {
    code: String,
    value: usize,
    group: Option<usize>,
}
fn entities(decimal: bool) -> Vec<Entity> {
    (0..7)
        .map(|i| Entity {
            code: format!("Generated Entity {i}"),
            value: i % 5,
            group: (!decimal).then_some(i % 3),
        })
        .collect()
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct CustomJustification<S> {
    score: S,
    facts: Vec<Entity>,
}
fn case<S: AdditiveScore + FromStr + 'static>(factor: i64, decimal: bool, custom: bool)
where
    S::Err: Debug,
{
    let entities = entities(decimal);
    let mut builder = StreamGraphBuilder::<S>::new();
    let source = builder.source::<usize, Entity>();
    // Native signed terminal contributions replace Java penalize/reward/impact builders.
    let per_match: S = factor.to_string().parse().unwrap();
    let contribution = per_match.clone();
    let constraint = builder
        .score("testConstraint", source.port(), move |_| {
            contribution.clone()
        })
        .unwrap();
    let mut graph = builder.build();
    let mut tx = graph.transaction();
    for (i, entity) in entities.iter().enumerate() {
        tx.apply(&source, TupleChange::put(i, entity.clone()))
            .unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(
        graph.score(),
        (7 * factor).to_string().parse::<S>().unwrap()
    );
    if custom {
        let report = graph
            .explain_constraint(&constraint, |entity, score| {
                (
                    CustomJustification {
                        score: score.clone(),
                        facts: vec![entity.clone()],
                    },
                    vec![entity.clone()],
                )
            })
            .unwrap();
        let mut totals = ScoreExplanation::default();
        totals.add_constraint(report).unwrap();
        assert_eq!(
            totals
                .constraints
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["testConstraint"]
        );
        let report = &totals.constraints["testConstraint"];
        assert_eq!(report.matches.len(), entities.len());
        for (i, entity) in entities.iter().enumerate() {
            let justification: &CustomJustification<S> = &report.matches[&i].justification;
            assert_eq!(justification.facts.as_slice(), std::slice::from_ref(entity));
            assert_eq!(justification.score, per_match);
        }
        assert_eq!(totals.score, graph.score());
    } else {
        let report = graph
            .explain_constraint_default(&constraint, |e| vec![e.clone()])
            .unwrap();
        let mut totals = ScoreExplanation::default();
        totals.add_constraint(report).unwrap();
        assert_eq!(
            totals
                .constraints
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["testConstraint"]
        );
        let report = &totals.constraints["testConstraint"];
        assert_eq!(report.matches.len(), entities.len());
        for (i, entity) in entities.iter().enumerate() {
            let justification: &DefaultJustification<S, Entity> = &report.matches[&i].justification;
            assert_eq!(justification.facts.as_slice(), std::slice::from_ref(entity));
            assert_eq!(justification.score, per_match);
        }
        assert_eq!(totals.score, graph.score());
    }
}

#[test]
fn penalize_unweighted() {
    case::<SimpleScore>(-1, false, false);
}

#[test]
fn penalize_unweighted_big_decimal() {
    case::<SimpleDecimalScore>(-1, true, false);
}

#[test]
fn penalize() {
    case::<SimpleScore>(-2, false, false);
}

#[test]
fn penalize_big_decimal() {
    case::<SimpleDecimalScore>(-2, true, false);
}

#[test]
fn reward_unweighted() {
    case::<SimpleScore>(1, false, false);
}

#[test]
fn reward() {
    case::<SimpleScore>(2, false, false);
}

#[test]
fn reward_big_decimal() {
    case::<SimpleDecimalScore>(2, true, false);
}

#[test]
fn impact_positive_unweighted() {
    case::<SimpleScore>(1, false, false);
}

#[test]
fn impact_positive() {
    case::<SimpleScore>(2, false, false);
}

#[test]
fn impact_positive_big_decimal() {
    case::<SimpleDecimalScore>(2, true, false);
}

#[test]
fn impact_negative() {
    case::<SimpleScore>(-2, false, false);
}

#[test]
fn impact_negative_big_decimal() {
    case::<SimpleDecimalScore>(-2, true, false);
}

#[test]
fn penalize_unweighted_custom_justifications() {
    case::<SimpleScore>(-1, false, true);
}

#[test]
fn penalize_custom_justifications() {
    case::<SimpleScore>(-2, false, true);
}

#[test]
fn penalize_big_decimal_custom_justifications() {
    case::<SimpleDecimalScore>(-2, true, true);
}

#[test]
fn reward_unweighted_custom_justifications() {
    case::<SimpleScore>(1, false, true);
}

#[test]
fn reward_custom_justifications() {
    case::<SimpleScore>(2, false, true);
}

#[test]
fn reward_big_decimal_custom_justifications() {
    case::<SimpleDecimalScore>(2, true, true);
}

#[test]
fn impact_positive_unweighted_custom_justifications() {
    case::<SimpleScore>(1, false, true);
}

#[test]
fn impact_positive_custom_justifications() {
    case::<SimpleScore>(2, false, true);
}

#[test]
fn impact_positive_big_decimal_custom_justifications() {
    case::<SimpleDecimalScore>(2, true, true);
}

#[test]
fn impact_negative_custom_justifications() {
    case::<SimpleScore>(-2, false, true);
}

#[test]
fn impact_negative_big_decimal_custom_justifications() {
    case::<SimpleDecimalScore>(-2, true, true);
}
