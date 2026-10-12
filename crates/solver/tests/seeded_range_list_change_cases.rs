// Complete adapted Community random range ListChangeMoveSelectorTest cases.
// Apache-2.0; TEST setup/filter and probe provenance recorded separately.
use std::{num::NonZeroUsize, sync::Arc};
use timetuck::*;
fn fixture(filtered: bool, optional: bool) -> (Arc<ListProblem<i32>>, ListAssignment<i32>) {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![2, 1]).with_value_range(if filtered {
                vec![1, 2, 3]
            } else {
                vec![1, 2]
            }),
            ListEntity::new(vec![3]).with_value_range(vec![2, 3]),
        ],
    )
    .unwrap();
    let p = Arc::new(if optional { p.allowing_unassigned() } else { p });
    let a = p.initial_assignment();
    (p, a)
}
fn limits() -> CorrelatedListChangeLimits {
    CorrelatedListChangeLimits {
        values_per_item: NonZeroUsize::new(64).unwrap(),
        destinations_per_value: NonZeroUsize::new(64).unwrap(),
    }
}
fn position(p: Option<ListPosition>) -> String {
    p.map_or_else(
        || "null".into(),
        |p| format!("{}[{}]", char::from(b'A' + p.entity as u8), p.index),
    )
}
fn assert_trace<F>(
    mut source: SeededEntityRangeListChangeSource<F>,
    c: &(Arc<ListProblem<i32>>, ListAssignment<i32>),
    want: &[&str],
) where
    F: EntityRangeAnchorFilter<(Arc<ListProblem<i32>>, ListAssignment<i32>), i32>,
{
    source.event(SelectionLifecycleEvent::SolveStarted, c);
    source.event(SelectionLifecycleEvent::PhaseStarted, c);
    assert!(<SeededEntityRangeListChangeSource<F> as SelectionSource<
        _,
        ListValueChange<i32>,
    >>::is_never_ending(&source));
    let mut it = source.open(c).peekable();
    for want in want {
        assert!(it.peek().is_some());
        let v = it.next().unwrap();
        assert_eq!(
            format!(
                "{} {{{}->{}}}",
                v.value(),
                position(v.source()),
                position(v.destination())
            ),
            *want
        );
    }
    assert!(it.peek().is_some());
}
#[test]
fn random_with_entity_value_range() {
    assert_trace(
        SeededEntityRangeListChangeSource::new(0, limits(), true),
        &fixture(false, false),
        &[
            "1 {A[1]->A[1]}",
            "3 {B[0]->B[0]}",
            "3 {B[0]->B[0]}",
            "3 {B[0]->B[1]}",
            "3 {B[0]->B[1]}",
        ],
    );
}
#[test]
fn random_with_entity_value_range_and_filtering() {
    let filter = |c: &(Arc<ListProblem<i32>>, ListAssignment<i32>), v: &i32| {
        c.1.location(v).is_some_and(|p| p.entity != 1)
    };
    assert_trace(
        SeededEntityRangeListChangeSource::with_anchor_filter(0, limits(), true, filter),
        &fixture(true, false),
        &[
            "1 {A[1]->A[1]}",
            "3 {B[0]->A[0]}",
            "3 {B[0]->B[0]}",
            "3 {B[0]->A[2]}",
            "3 {B[0]->A[1]}",
        ],
    );
}
#[test]
fn random_allows_unassigned_values_with_entity_value_range() {
    assert_trace(
        SeededEntityRangeListChangeSource::new(0, limits(), true),
        &fixture(false, true),
        &[
            "1 {A[1]->A[1]}",
            "3 {B[0]->B[1]}",
            "3 {B[0]->B[1]}",
            "3 {B[0]->B[1]}",
            "3 {B[0]->B[1]}",
        ],
    );
}
