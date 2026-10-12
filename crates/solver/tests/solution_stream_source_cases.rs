// Complete selected Community unknownClass source-method adaptations.
// Native checked source handles replace reflective class-to-stream binding.
// Apache-2.0; exact source paths and changes in compat/provenance.json.
#[path = "fixtures/solution_stream_lavish_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
fn diagnostic(result: Result<(), SolutionStreamError>) {
    let error = result.expect_err("unknown source class must fail during graph construction");
    assert!(
        matches!(error,SolutionStreamError::UnknownType {requested,..} if requested.is::<i32>())
    );
    assert!(error.to_string().contains(PlanningType::of::<i32>().name()));
    assert!(error.to_string().contains("assignable from"));
}
fn existence<I: Ord + Clone + 'static, T: Clone + 'static>(
    registry: &SolutionStreamRegistry,
    b: &mut StreamGraphBuilder<'_, SimpleScore>,
    input: &StreamPort<I, T>,
    mode: ExistenceMode,
) -> Result<(), SolutionStreamError> {
    let unknown = registry.source::<i32, _>(b)?;
    let output = b
        .existence_equal(input, unknown.port(), mode, |_| (), |_| ())
        .unwrap();
    b.score("TEST", &output, |_| SimpleScore::new(-1)).unwrap();
    Ok(())
}
fn uni(mode: ExistenceMode) -> Result<(), SolutionStreamError> {
    let r = registry();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let input = r.source::<ValueGroup, _>(&mut b)?;
    existence(&r, &mut b, input.port(), mode)
}
fn bi(mode: ExistenceMode) -> Result<(), SolutionStreamError> {
    let r = registry();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let entities = r.source::<Entity, _>(&mut b)?;
    let input = b.unique_pairs(entities.port()).unwrap();
    existence(&r, &mut b, &input, mode)
}
fn tri(mode: ExistenceMode) -> Result<(), SolutionStreamError> {
    let r = registry();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let groups = r.source::<ValueGroup, _>(&mut b)?;
    let pair = b.unique_pairs(groups.port()).unwrap();
    let other = r.source::<EntityGroup, _>(&mut b)?;
    let input = b.join_equal(&pair, other.port(), |_| (), |_| ()).unwrap();
    existence(&r, &mut b, &input, mode)
}
fn quad(mode: ExistenceMode) -> Result<(), SolutionStreamError> {
    let r = registry();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let entities = r.source::<Entity, _>(&mut b)?;
    let groups = r.source::<EntityGroup, _>(&mut b)?;
    let values = r.source::<Value, _>(&mut b)?;
    let pair = b
        .join_equal(
            entities.port(),
            groups.port(),
            |e| e.get().group.as_ref().map(arc_id),
            |g| Some(object_id(g)),
        )
        .unwrap();
    let triple = b
        .join_equal(
            &pair,
            values.port(),
            |(e, _)| e.get().value.as_ref().map(arc_id),
            |v| Some(object_id(v)),
        )
        .unwrap();
    let input = b
        .join_equal(
            &triple,
            entities.port(),
            |((_, g), _)| Some(object_id(g)),
            |e| e.get().group.as_ref().map(arc_id),
        )
        .unwrap();
    existence(&r, &mut b, &input, mode)
}
#[test]
fn uni_join_unknown_class() {
    diagnostic((|| {
        let r = registry();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let groups = r.source::<ValueGroup, _>(&mut b)?;
        let unknown = r.source::<i32, _>(&mut b)?;
        let joined = b
            .join_equal(groups.port(), unknown.port(), |_| (), |_| ())
            .unwrap();
        b.score("TEST", &joined, |_| SimpleScore::new(-1)).unwrap();
        Ok(())
    })());
}
#[test]
fn uni_for_each_unknown_class() {
    diagnostic((|| {
        let r = registry();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let unknown = r.source::<i32, _>(&mut b)?;
        b.score("TEST", unknown.port(), |_| SimpleScore::new(-1))
            .unwrap();
        Ok(())
    })());
}
#[test]
fn uni_if_exists_unknown_class() {
    diagnostic(uni(ExistenceMode::Exists));
}
#[test]
fn uni_if_not_exists_unknown_class() {
    diagnostic(uni(ExistenceMode::NotExists));
}
#[test]
fn bi_if_exists_unknown_class() {
    diagnostic(bi(ExistenceMode::Exists));
}
#[test]
fn bi_if_not_exists_unknown_class() {
    diagnostic(bi(ExistenceMode::NotExists));
}
#[test]
fn tri_if_exists_unknown_class() {
    diagnostic(tri(ExistenceMode::Exists));
}
#[test]
fn tri_if_not_exists_unknown_class() {
    diagnostic(tri(ExistenceMode::NotExists));
}
#[test]
fn quad_if_exists_unknown_class() {
    diagnostic(quad(ExistenceMode::Exists));
}
#[test]
fn quad_if_not_exists_unknown_class() {
    diagnostic(quad(ExistenceMode::NotExists));
}
