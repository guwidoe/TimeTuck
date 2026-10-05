// Adapted from pinned Timefold Community AbstractBi/Tri/QuadConstraintStreamTest.
// Apache-2.0; source identities, fixtures and substitutions: compat/provenance.json.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use timetuck::*;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Entity {
    code: String,
    value: usize,
    group: Option<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Value {
    id: usize,
    code: String,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Fact {
    Entity(Entity),
    Value(Value),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct CustomJustification<S> {
    score: S,
    facts: Vec<Fact>,
}

trait CaseScore: AdditiveScore + 'static {
    const DECIMAL: bool;
    fn number(value: i64) -> Self;
    fn terminal<I: Ord + Clone + 'static>(
        builder: &mut StreamGraphBuilder<'_, Self>,
        port: &StreamPort<I, Vec<Fact>>,
        factor: i64,
    ) -> StreamConstraint<I, Vec<Fact>>;
}
impl CaseScore for SimpleScore {
    const DECIMAL: bool = false;
    fn number(value: i64) -> Self {
        value.to_string().parse().unwrap()
    }
    fn terminal<I: Ord + Clone + 'static>(
        builder: &mut StreamGraphBuilder<'_, Self>,
        port: &StreamPort<I, Vec<Fact>>,
        factor: i64,
    ) -> StreamConstraint<I, Vec<Fact>> {
        builder
            .score_weighted("testConstraint", port, Self::number(1), move |_| factor)
            .unwrap()
    }
}
impl CaseScore for SimpleDecimalScore {
    const DECIMAL: bool = true;
    fn number(value: i64) -> Self {
        value.to_string().parse().unwrap()
    }
    fn terminal<I: Ord + Clone + 'static>(
        builder: &mut StreamGraphBuilder<'_, Self>,
        port: &StreamPort<I, Vec<Fact>>,
        factor: i64,
    ) -> StreamConstraint<I, Vec<Fact>> {
        let factor: Decimal = factor.to_string().parse().unwrap();
        builder
            .score_weighted_decimal("testConstraint", port, Self::number(1), move |_| {
                factor.clone()
            })
            .unwrap()
    }
}

#[derive(Clone, Copy)]
struct Spec {
    arity: usize,
    factor: i64,
    total: i64,
    custom: bool,
}
fn case<S: CaseScore>(arity: usize, factor: i64, total: i64, custom: bool) {
    let spec = Spec {
        arity,
        factor,
        total,
        custom,
    };
    let mut builder = StreamGraphBuilder::<S>::new();
    let entities = builder.source::<usize, Entity>();
    let values = builder.source::<usize, Value>();
    let pairs = if arity == 2 {
        builder.unique_pairs(entities.port()).unwrap()
    } else {
        builder
            .unique_pairs_equal(entities.port(), |e| e.value)
            .unwrap()
    };
    if arity == 2 {
        let rows = builder
            .map(&pairs, |(a, b)| {
                vec![Fact::Entity(a.clone()), Fact::Entity(b.clone())]
            })
            .unwrap();
        finish(builder, (entities, values), rows, spec);
    } else {
        let triples = builder
            .join_equal(&pairs, values.port(), |(a, _)| a.value, |v| v.id)
            .unwrap();
        if arity == 3 {
            let rows = builder
                .map(&triples, |((a, b), c)| {
                    vec![
                        Fact::Entity(a.clone()),
                        Fact::Entity(b.clone()),
                        Fact::Value(c.clone()),
                    ]
                })
                .unwrap();
            finish(builder, (entities, values), rows, spec);
        } else {
            assert_eq!(arity, 4);
            let quads = builder
                .join_equal(&triples, values.port(), |(_, c)| c.id, |v| v.id)
                .unwrap();
            let rows = builder
                .map(&quads, |(((a, b), c), d)| {
                    vec![
                        Fact::Entity(a.clone()),
                        Fact::Entity(b.clone()),
                        Fact::Value(c.clone()),
                        Fact::Value(d.clone()),
                    ]
                })
                .unwrap();
            finish(builder, (entities, values), rows, spec);
        }
    }
}

fn finish<I: Ord + Clone + Debug + 'static, S: CaseScore>(
    mut builder: StreamGraphBuilder<'_, S>,
    sources: (StreamSource<usize, Entity>, StreamSource<usize, Value>),
    rows: StreamPort<I, Vec<Fact>>,
    spec: Spec,
) {
    let entities: Vec<_> = (0..7)
        .map(|i| Entity {
            code: format!("Generated Entity {i}"),
            value: i % 5,
            group: (!S::DECIMAL).then_some(i % 3),
        })
        .collect();
    let values: Vec<_> = (0..5)
        .map(|id| Value {
            id,
            code: format!("Generated Value {id}"),
        })
        .collect();
    let constraint = S::terminal(&mut builder, &rows, spec.factor);
    let mut graph = builder.build();
    let mut tx = graph.transaction();
    for (i, value) in values.iter().enumerate() {
        tx.apply(&sources.1, TupleChange::put(i, value.clone()))
            .unwrap();
    }
    for (i, entity) in entities.iter().enumerate() {
        tx.apply(&sources.0, TupleChange::put(i, entity.clone()))
            .unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(graph.score(), S::number(spec.total));
    assert_eq!(
        graph.constraint_scores().collect::<Vec<_>>(),
        [("testConstraint", S::number(spec.total))]
    );

    // Independently enumerate all pairs, or the two literal equal-value pairs.
    let pairs: Vec<_> = if spec.arity == 2 {
        (0..7)
            .flat_map(|a| (a + 1..7).map(move |b| (a, b)))
            .collect()
    } else {
        vec![(0, 5), (1, 6)]
    };
    let expected: Vec<Vec<Fact>> = pairs
        .into_iter()
        .map(|(a, b)| {
            let mut facts = vec![
                Fact::Entity(entities[a].clone()),
                Fact::Entity(entities[b].clone()),
            ];
            if spec.arity >= 3 {
                facts.push(Fact::Value(values[a].clone()));
            }
            if spec.arity == 4 {
                facts.push(Fact::Value(values[a].clone()));
            }
            facts
        })
        .collect();
    if spec.custom {
        let report = graph
            .explain_constraint(&constraint, |facts, score| {
                (
                    CustomJustification {
                        score: score.clone(),
                        facts: facts.clone(),
                    },
                    facts.clone(),
                )
            })
            .unwrap();
        assert_explanation(report, spec, expected, |j: &CustomJustification<S>| {
            (&j.score, &j.facts)
        });
    } else {
        let report = graph
            .explain_constraint_default(&constraint, Clone::clone)
            .unwrap();
        assert_explanation(
            report,
            spec,
            expected,
            |j: &DefaultJustification<S, Fact>| (&j.score, &j.facts),
        );
    }
}

fn assert_explanation<I: Ord + Clone + Debug, S: CaseScore, J>(
    report: ConstraintExplanation<I, S, J, Fact>,
    spec: Spec,
    mut expected: Vec<Vec<Fact>>,
    fields: impl Fn(&J) -> (&S, &[Fact]),
) {
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
    assert_eq!(totals.score, S::number(spec.total));
    let report = &totals.constraints["testConstraint"];
    assert_eq!(report.score, S::number(spec.total));
    assert_eq!(report.matches.len(), if spec.arity == 2 { 21 } else { 2 });
    let mut actual = Vec::new();
    for entry in report.matches.values() {
        let (score, facts) = fields(&entry.justification);
        assert_eq!(score, &S::number(spec.factor));
        assert_eq!(entry.score, S::number(spec.factor));
        assert_eq!(facts.len(), spec.arity);
        assert_eq!(
            entry.indicted_facts,
            facts.iter().cloned().collect::<BTreeSet<_>>()
        );
        actual.push(facts.to_vec());
    }
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    // Repeated value facts in a quad remain in the justification, but each match
    // indicts a value only once. Expected counts come from the independent rows.
    let mut counts = BTreeMap::<Fact, i64>::new();
    for row in expected {
        for fact in row.into_iter().collect::<BTreeSet<_>>() {
            *counts.entry(fact).or_default() += 1;
        }
    }
    assert_eq!(report.indictments.len(), counts.len());
    assert_eq!(totals.indictments.len(), counts.len());
    for (fact, count) in counts {
        let indictment = &report.indictments[&fact];
        assert_eq!(indictment.matches.len(), count as usize);
        assert_eq!(indictment.score, S::number(count * spec.factor));
        assert_eq!(totals.indictments[&fact].score, indictment.score);
        assert_eq!(totals.indictments[&fact].matches.len(), count as usize);
    }
}

#[test]
fn bi_penalize_unweighted() {
    case::<SimpleScore>(2, -1, -21, false);
}

#[test]
fn bi_penalize_unweighted_big_decimal() {
    case::<SimpleDecimalScore>(2, -1, -21, false);
}

#[test]
fn bi_penalize() {
    case::<SimpleScore>(2, -2, -42, false);
}

#[test]
fn bi_penalize_big_decimal() {
    case::<SimpleDecimalScore>(2, -2, -42, false);
}

#[test]
fn bi_reward_unweighted() {
    case::<SimpleScore>(2, 1, 21, false);
}

#[test]
fn bi_reward() {
    case::<SimpleScore>(2, 2, 42, false);
}

#[test]
fn bi_reward_big_decimal() {
    case::<SimpleDecimalScore>(2, 2, 42, false);
}

#[test]
fn bi_impact_positive_unweighted() {
    case::<SimpleScore>(2, 1, 21, false);
}

#[test]
fn bi_impact_positive() {
    case::<SimpleScore>(2, 2, 42, false);
}

#[test]
fn bi_impact_positive_big_decimal() {
    case::<SimpleDecimalScore>(2, 2, 42, false);
}

#[test]
fn bi_impact_negative() {
    case::<SimpleScore>(2, -2, -42, false);
}

#[test]
fn bi_impact_negative_big_decimal() {
    case::<SimpleDecimalScore>(2, -2, -42, false);
}

#[test]
fn bi_penalize_unweighted_custom_justifications() {
    case::<SimpleScore>(2, -1, -21, true);
}

#[test]
fn bi_penalize_custom_justifications() {
    case::<SimpleScore>(2, -2, -42, true);
}

#[test]
fn bi_penalize_big_decimal_custom_justifications() {
    case::<SimpleDecimalScore>(2, -2, -42, true);
}

#[test]
fn bi_reward_unweighted_custom_justifications() {
    case::<SimpleScore>(2, 1, 21, true);
}

#[test]
fn bi_reward_custom_justifications() {
    case::<SimpleScore>(2, 2, 42, true);
}

#[test]
fn bi_reward_big_decimal_custom_justifications() {
    case::<SimpleDecimalScore>(2, 2, 42, true);
}

#[test]
fn bi_impact_positive_unweighted_custom_justifications() {
    case::<SimpleScore>(2, 1, 21, true);
}

#[test]
fn bi_impact_positive_custom_justifications() {
    case::<SimpleScore>(2, 2, 42, true);
}

#[test]
fn bi_impact_positive_big_decimal_custom_justifications() {
    case::<SimpleDecimalScore>(2, 2, 42, true);
}

#[test]
fn bi_impact_negative_custom_justifications() {
    case::<SimpleScore>(2, -2, -42, true);
}

#[test]
fn bi_impact_negative_big_decimal_custom_justifications() {
    case::<SimpleDecimalScore>(2, -2, -42, true);
}

#[test]
fn tri_penalize_unweighted() {
    case::<SimpleScore>(3, -1, -2, false);
}

#[test]
fn tri_penalize_unweighted_big_decimal() {
    case::<SimpleDecimalScore>(3, -1, -2, false);
}

#[test]
fn tri_penalize() {
    case::<SimpleScore>(3, -2, -4, false);
}

#[test]
fn tri_penalize_big_decimal() {
    case::<SimpleDecimalScore>(3, -2, -4, false);
}

#[test]
fn tri_reward_unweighted() {
    case::<SimpleScore>(3, 1, 2, false);
}

#[test]
fn tri_reward() {
    case::<SimpleScore>(3, 2, 4, false);
}

#[test]
fn tri_reward_big_decimal() {
    case::<SimpleDecimalScore>(3, 2, 4, false);
}

#[test]
fn tri_impact_positive_unweighted() {
    case::<SimpleScore>(3, 1, 2, false);
}

#[test]
fn tri_impact_positive() {
    case::<SimpleScore>(3, 2, 4, false);
}

#[test]
fn tri_impact_positive_big_decimal() {
    case::<SimpleDecimalScore>(3, 2, 4, false);
}

#[test]
fn tri_impact_negative() {
    case::<SimpleScore>(3, -2, -4, false);
}

#[test]
fn tri_impact_negative_big_decimal() {
    case::<SimpleDecimalScore>(3, -2, -4, false);
}

#[test]
fn tri_penalize_unweighted_custom_justifications() {
    case::<SimpleScore>(3, -1, -2, true);
}

#[test]
fn tri_penalize_custom_justifications() {
    case::<SimpleScore>(3, -2, -4, true);
}

#[test]
fn tri_penalize_big_decimal_custom_justifications() {
    case::<SimpleDecimalScore>(3, -2, -4, true);
}

#[test]
fn tri_reward_unweighted_custom_justifications() {
    case::<SimpleScore>(3, 1, 2, true);
}

#[test]
fn tri_reward_custom_justifications() {
    case::<SimpleScore>(3, 2, 4, true);
}

#[test]
fn tri_reward_big_decimal_custom_justifications() {
    case::<SimpleDecimalScore>(3, 2, 4, true);
}

#[test]
fn tri_impact_positive_unweighted_custom_justifications() {
    case::<SimpleScore>(3, 1, 2, true);
}

#[test]
fn tri_impact_positive_custom_justifications() {
    case::<SimpleScore>(3, 2, 4, true);
}

#[test]
fn tri_impact_positive_big_decimal_custom_justifications() {
    case::<SimpleDecimalScore>(3, 2, 4, true);
}

#[test]
fn tri_impact_negative_custom_justifications() {
    case::<SimpleScore>(3, -2, -4, true);
}

#[test]
fn tri_impact_negative_big_decimal_custom_justifications() {
    case::<SimpleDecimalScore>(3, -2, -4, true);
}

#[test]
fn quad_penalize_unweighted() {
    case::<SimpleScore>(4, -1, -2, false);
}

#[test]
fn quad_penalize_unweighted_big_decimal() {
    case::<SimpleDecimalScore>(4, -1, -2, false);
}

#[test]
fn quad_penalize() {
    case::<SimpleScore>(4, -2, -4, false);
}

#[test]
fn quad_penalize_big_decimal() {
    case::<SimpleDecimalScore>(4, -2, -4, false);
}

#[test]
fn quad_reward_unweighted() {
    case::<SimpleScore>(4, 1, 2, false);
}

#[test]
fn quad_reward() {
    case::<SimpleScore>(4, 2, 4, false);
}

#[test]
fn quad_reward_big_decimal() {
    case::<SimpleDecimalScore>(4, 2, 4, false);
}

#[test]
fn quad_impact_positive_unweighted() {
    case::<SimpleScore>(4, 1, 2, false);
}

#[test]
fn quad_impact_positive() {
    case::<SimpleScore>(4, 2, 4, false);
}

#[test]
fn quad_impact_positive_big_decimal() {
    case::<SimpleDecimalScore>(4, 2, 4, false);
}

#[test]
fn quad_impact_negative() {
    case::<SimpleScore>(4, -2, -4, false);
}

#[test]
fn quad_impact_negative_big_decimal() {
    case::<SimpleDecimalScore>(4, -2, -4, false);
}

#[test]
fn quad_penalize_unweighted_custom_justifications() {
    case::<SimpleScore>(4, -1, -2, true);
}

#[test]
fn quad_penalize_custom_justifications() {
    case::<SimpleScore>(4, -2, -4, true);
}

#[test]
fn quad_penalize_big_decimal_custom_justifications() {
    case::<SimpleDecimalScore>(4, -2, -4, true);
}

#[test]
fn quad_reward_unweighted_custom_justifications() {
    case::<SimpleScore>(4, 1, 2, true);
}

#[test]
fn quad_reward_custom_justifications() {
    case::<SimpleScore>(4, 2, 4, true);
}

#[test]
fn quad_reward_big_decimal_custom_justifications() {
    case::<SimpleDecimalScore>(4, 2, 4, true);
}

#[test]
fn quad_impact_positive_unweighted_custom_justifications() {
    case::<SimpleScore>(4, 1, 2, true);
}

#[test]
fn quad_impact_positive_custom_justifications() {
    case::<SimpleScore>(4, 2, 4, true);
}

#[test]
fn quad_impact_positive_big_decimal_custom_justifications() {
    case::<SimpleDecimalScore>(4, 2, 4, true);
}

#[test]
fn quad_impact_negative_custom_justifications() {
    case::<SimpleScore>(4, -2, -4, true);
}

#[test]
fn quad_impact_negative_big_decimal_custom_justifications() {
    case::<SimpleDecimalScore>(4, -2, -4, true);
}
