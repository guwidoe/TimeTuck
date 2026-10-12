// Complete selected Community construction-error adaptations; Apache-2.0.
#[path = "fixtures/custom_justification_fixture.rs"]
mod justifications;
#[path = "fixtures/solution_stream_lavish_fixture.rs"]
mod lavish;
use justifications::*;
use lavish::*;
use std::{any::Any, sync::Arc};
use timetuck::*;
fn duplicate<
    'a,
    I: Ord + Clone + 'static,
    T: Clone + 'static,
    F: Fn(&T) -> Vec<Arc<dyn Any + Send + Sync>> + Clone + 'a,
>(
    builder: &mut StreamGraphBuilder<'a, SimpleScore>,
    input: &StreamPort<I, T>,
    facts: F,
) -> Result<(), ConstraintDeclarationError> {
    let mut declaration =
        CustomConstraintDeclaration::new(input, "TEST", SimpleScore::new(-1), |_| 2);
    let first = facts.clone();
    declaration.justify_with(move |row, score| TestJustification {
        score: *score,
        facts: first(row),
    })?;
    declaration.justify_with(move |row, score| TestJustification {
        score: *score,
        facts: facts(row),
    })?;
    declaration.build(builder)?;
    Ok(())
}
fn assert_duplicate(result: Result<(), ConstraintDeclarationError>) {
    let error = result.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Maybe the constraint calls justifyWith() twice?")
    );
}
#[test]
fn uni_fail_with_multiple_justifications() {
    let registry = registry();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = registry.source::<Entity, _>(&mut b).unwrap();
    assert_duplicate(duplicate(&mut b, source.port(), |e| {
        vec![e.allocation().clone()]
    }));
}
#[test]
fn bi_fail_with_multiple_justifications() {
    let registry = registry();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = registry.source::<Entity, _>(&mut b).unwrap();
    let pairs = b.unique_pairs(source.port()).unwrap();
    assert_duplicate(duplicate(&mut b, &pairs, |(a, b)| {
        vec![a.allocation().clone(), b.allocation().clone()]
    }));
}
#[test]
fn tri_fail_with_multiple_justifications() {
    let registry = registry();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = registry.source::<Entity, _>(&mut b).unwrap();
    let pairs = b
        .unique_pairs_equal(source.port(), |e| e.get().value.as_ref().map(arc_id))
        .unwrap();
    let values = registry.source::<Value, _>(&mut b).unwrap();
    let triples = b
        .join_equal(
            &pairs,
            values.port(),
            |(a, _)| a.get().value.as_ref().map(arc_id),
            |v| Some(object_id(v)),
        )
        .unwrap();
    assert_duplicate(duplicate(&mut b, &triples, |((a, b), c)| {
        vec![
            a.allocation().clone(),
            b.allocation().clone(),
            c.allocation().clone(),
        ]
    }));
}
#[test]
fn quad_fail_with_multiple_justifications() {
    let registry = registry();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = registry.source::<Entity, _>(&mut b).unwrap();
    let pairs = b
        .unique_pairs_equal(source.port(), |e| e.get().value.as_ref().map(arc_id))
        .unwrap();
    let values = registry.source::<Value, _>(&mut b).unwrap();
    let triples = b
        .join_equal(
            &pairs,
            values.port(),
            |(a, _)| a.get().value.as_ref().map(arc_id),
            |v| Some(object_id(v)),
        )
        .unwrap();
    let quads = b
        .join_equal(&triples, values.port(), |(_, v)| object_id(v), object_id)
        .unwrap();
    assert_duplicate(duplicate(&mut b, &quads, |(((a, b), c), d)| {
        vec![
            a.allocation().clone(),
            b.allocation().clone(),
            c.allocation().clone(),
            d.allocation().clone(),
        ]
    }));
}
#[test]
fn bi_join_filtering_comes_last() {
    let registry = registry();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let entities = registry.source::<Entity, _>(&mut b).unwrap();
    let values = registry.source::<Value, _>(&mut b).unwrap();
    let left =
        JoinProjection::new(|e: &SolutionStreamObject<Entity>| e.get().value.as_ref().map(arc_id));
    let right = JoinProjection::new(|v: &SolutionStreamObject<Value>| Some(object_id(v)));
    let conditions = OrderedJoinConditions::new()
        .filtering(|_, _| false)
        .indexed(JoinCondition::new(JoinComparison::Equal, &left, &right));
    let result = (|| {
        let pair = conditions.build(&mut b, entities.port(), values.port())?;
        let extras = registry.source::<Extra, _>(&mut b).unwrap();
        let triples = b.join_equal(&pair, extras.port(), |_| (), |_| ()).unwrap();
        b.score("TEST", &triples, |_| SimpleScore::new(-1)).unwrap();
        Ok::<(), OrderedJoinError>(())
    })();
    assert_eq!(result.unwrap_err(), OrderedJoinError::IndexedAfterFiltering);
}
