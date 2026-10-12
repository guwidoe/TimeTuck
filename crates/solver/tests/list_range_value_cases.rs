// Complete adapted Community ListChangeMoveSelectorTest pin/entity-range case.
// Apache-2.0; source fixtures/helpers are recorded in compat/provenance.json.
use std::{num::NonZeroUsize, sync::Arc};
use timetuck::*;
struct Destinations(Vec<Option<ListPosition>>);
impl<C> SelectionSource<C, Option<ListPosition>> for Destinations {
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.0.len())
    }
    fn is_never_ending(&self) -> bool {
        true
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = Option<ListPosition>> + 'a>
    where
        Option<ListPosition>: 'a,
    {
        Box::new(self.0.iter().copied().cycle())
    }
}
fn position(p: Option<ListPosition>) -> String {
    p.map_or_else(
        || "null".into(),
        |p| format!("{}[{}]", char::from(b'A' + p.entity as u8), p.index),
    )
}
#[test]
fn random_with_pinning_and_entity_value_range() {
    let p = Arc::new(
        ListProblem::new(
            vec![1, 2, 3, 4],
            vec![
                ListEntity::new(vec![2, 1])
                    .with_value_range(vec![1, 2, 3])
                    .with_pinned_prefix(1),
                ListEntity::new(vec![4])
                    .with_value_range(vec![2, 4])
                    .with_pinned(true),
                ListEntity::new(vec![3]).with_value_range(vec![3, 4]),
            ],
        )
        .unwrap(),
    );
    let c = (p.clone(), p.initial_assignment());
    let dest = |entity, index| Some(ListPosition { entity, index });
    let mut source = ListValueChangeSource::new(
        ListRangeValueSource::new(ListRangeValueOrder::Lcg48Random { seed: 0 }),
        Destinations(vec![dest(2, 0), dest(0, 2), dest(0, 1), dest(2, 0)]),
        ListChangeSourceLimits {
            reads_per_side: NonZeroUsize::new(32).unwrap(),
            pairs_per_item: NonZeroUsize::new(32).unwrap(),
        },
        true,
    );
    source.event(SelectionLifecycleEvent::SolveStarted, &c);
    source.event(SelectionLifecycleEvent::PhaseStarted, &c);
    assert!(
        <ListValueChangeSource<ListRangeValueSource, Destinations> as SelectionSource<
            (Arc<ListProblem<i32>>, ListAssignment<i32>),
            ListValueChange<i32>,
        >>::is_never_ending(&source)
    );
    let mut it = source.open(&c).peekable();
    for want in ["3 {C[0]->C[0]}", "1 {A[1]->A[2]}"] {
        let item = it.next().unwrap();
        assert_eq!(
            format!(
                "{} {{{}->{}}}",
                item.value(),
                position(item.source()),
                position(item.destination())
            ),
            want
        );
    }
    assert!(it.peek().is_some());
}
