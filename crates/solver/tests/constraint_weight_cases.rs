// Adapted from pinned Timefold Community tests; Apache-2.0.
// Exact methods, fixtures and substitutions: compat/provenance.json.
use std::cell::Cell;
use timetuck::*;
const FIRST: &str = "First weight";
const SECOND: &str = "Second weight";
const THIRD: &str = "Third weight";
fn overrides(entries: &[(&str, i64)]) -> ConstraintWeightOverrides<SimpleScore> {
    entries
        .iter()
        .map(|(name, n)| ((*name).to_owned(), SimpleScore::new(*n)))
        .collect()
}
#[test]
fn consistent_order_of_constraints() {
    let a = overrides(&[(FIRST, 0), (SECOND, 1)]);
    let b = overrides(&[(SECOND, 1), (FIRST, 0)]);
    assert_eq!(
        a.iter().map(|(name, _)| name).collect::<Vec<_>>(),
        b.iter().map(|(name, _)| name).collect::<Vec<_>>()
    );
}
#[test]
fn reads_overrides_from_solution() {
    let mut weights = ConstraintWeightOverrides::default();
    assert_eq!(weights.get(FIRST), None);
    assert_eq!(weights.get(SECOND), None);
    weights = overrides(&[(SECOND, 1)]);
    assert_eq!(weights.get(FIRST), None);
    assert_eq!(weights.get(SECOND), Some(&SimpleScore::new(1)));
    weights = overrides(&[(FIRST, 0), (SECOND, 1)]);
    assert_eq!(weights.get(FIRST), Some(&SimpleScore::ZERO));
    assert_eq!(weights.get(SECOND), Some(&SimpleScore::new(1)));
}
fn score(weights: &ConstraintWeightOverrides<SimpleScore>) -> Result<SimpleScore, GraphError> {
    let mut b = StreamGraphBuilder::new();
    let entities = b.source::<usize, (String, usize)>();
    b.score_weighted(FIRST, entities.port(), SimpleScore::new(1), |_| 1)?;
    b.score_weighted(SECOND, entities.port(), SimpleScore::ZERO, |_| -1)?;
    let mut graph = b.build_with_weights(weights)?;
    let mut tx = graph.transaction();
    for i in 0..5 {
        tx.apply(
            &entities,
            TupleChange::put(i, (format!("Generated Entity {i}"), i % 3)),
        )?;
    }
    tx.commit()?;
    graph.assert_consistent()?;
    Ok(graph.score())
}
#[test]
fn applies_overrides_to_constraint_provider() {
    assert_eq!(
        score(&ConstraintWeightOverrides::default()).unwrap(),
        SimpleScore::new(5)
    );
    assert_eq!(
        score(&overrides(&[(FIRST, 0), (SECOND, 2)])).unwrap(),
        SimpleScore::new(-10)
    );
    let error = score(&overrides(&[(THIRD, 1)])).unwrap_err();
    assert!(matches!(error, GraphError::UnknownConstraintWeights { .. }));
    for name in [FIRST, SECOND, THIRD] {
        assert!(error.to_string().contains(name));
    }
}
#[test]
fn zero_constraint_weight_disabled() {
    // generateSolution(2,5,3,2), plus MyEntity 1 assigned to the first group/value.
    let entities = [
        ("Generated Entity 0", 0, 0, ""),
        ("Generated Entity 1", 1, 1, ""),
        ("MyEntity 1", 0, 0, "myProperty1"),
    ];
    let zero = Cell::new(0);
    let one = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (&str, usize, usize, &str)>();
    let disabled = b
        .filter(source.port(), |_| {
            zero.set(zero.get() + 1);
            true
        })
        .unwrap();
    let active = b
        .filter(source.port(), |_| {
            one.set(one.get() + 1);
            true
        })
        .unwrap();
    b.score_weighted("myConstraint1", &disabled, SimpleScore::ZERO, |_| -1)
        .unwrap();
    b.score_weighted("myConstraint2", &active, SimpleScore::new(1), |_| -1)
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for (i, e) in entities.into_iter().enumerate() {
        tx.apply(&source, TupleChange::put(i, e)).unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(zero.replace(0), 0);
    assert_eq!(one.replace(0), 3);
    let mut tx = g.transaction();
    tx.apply(
        &source,
        TupleChange::put(2, ("MyEntity 1", 0, 0, "myProperty2")),
    )
    .unwrap();
    tx.commit().unwrap();
    assert_eq!(zero.get(), 0);
    assert_eq!(one.get(), 1);
}
#[test]
fn duplicate_constraint_id() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, usize>();
    b.score_weighted(
        "duplicateConstraintName",
        source.port(),
        SimpleScore::new(1),
        |_| -1,
    )
    .unwrap();
    assert!(matches!(
        b.score_weighted(
            "duplicateConstraintName",
            source.port(),
            SimpleScore::new(1),
            |_| -1
        ),
        Err(GraphError::DuplicateConstraint(_))
    ));
}
