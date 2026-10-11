// Adapted from pinned Community ReachableValuesTest and OriginalList*IteratorTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{num::NonZeroUsize, sync::Arc};
use timetuck::*;
struct Finite<T>(Vec<T>);
impl<C, T: Clone> SelectionSource<C, T> for Finite<T> {
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.0.len())
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        Box::new(self.0.iter().cloned())
    }
}
fn limits() -> ReplayedListChangeLimits {
    ReplayedListChangeLimits {
        values_per_item: NonZeroUsize::new(8).unwrap(),
        destinations_per_value: NonZeroUsize::new(8).unwrap(),
        reads_per_child: NonZeroUsize::new(8).unwrap(),
    }
}
#[test]
fn reachable_values_by_entity() {
    let p = ListProblem::new(
        vec!["V1", "V2", "V3", "V4", "V5"],
        vec![
            ListEntity::new(vec!["V1", "V2"]).with_value_range(vec!["V1", "V2", "V3"]),
            ListEntity::new(vec!["V3"]).with_value_range(vec!["V2", "V3"]),
            ListEntity::new(vec!["V4", "V5"]).with_value_range(vec!["V3", "V4", "V5"]),
        ],
    )
    .unwrap();
    let mut views = ReachabilityViews::new(list_reachability(&p));
    let view = views.unsorted_view();
    let index = view.reachable_index();
    for (v, expected) in [
        ("V1", vec![0]),
        ("V2", vec![0, 1]),
        ("V3", vec![0, 1, 2]),
        ("V4", vec![2]),
        ("V5", vec![2]),
    ] {
        assert_eq!(index.entities(&v), expected);
    }
    assert!(index.is_entity_reachable(&"V1", &0));
    assert!(!index.is_entity_reachable(&"V1", &1));
    assert!(!index.is_entity_reachable(&"V1", &2));
    assert!(view.matches_value_payload(&"V1"));
    assert!(!view.matches_value_payload(&0usize));
}
#[test]
fn multiple_sorters() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec!["V2", "V3", "V1"]).allowing_unassigned(),
        Variable::new(vec!["V2", "V3"]).allowing_unassigned(),
        Variable::new(vec!["V2", "V3", "V4", "V5"]).allowing_unassigned(),
    ])
    .unwrap();
    let mut views = ReachabilityViews::new(basic_reachability(&p));
    let unsorted = views.unsorted_view();
    let mut initial = unsorted.ordered_co_values(&"V2");
    initial.sort();
    assert_eq!(initial, vec![&"V1", &"V3", &"V4", &"V5"]);
    let ascending = views.sorted_view(ReachabilityViewKey(1), Ord::cmp);
    assert!(!Arc::ptr_eq(&ascending, &unsorted));
    assert!(Arc::ptr_eq(
        &ascending,
        &views.sorted_view(ReachabilityViewKey(1), Ord::cmp)
    ));
    assert_eq!(
        unsorted.ordered_co_values(&"V2"),
        vec![&"V1", &"V3", &"V4", &"V5"]
    );
    let descending = views.sorted_view(ReachabilityViewKey(2), |a, b| b.cmp(a));
    assert!(!Arc::ptr_eq(&descending, &ascending));
    assert!(!Arc::ptr_eq(&descending, &unsorted));
    assert!(Arc::ptr_eq(
        &descending,
        &views.sorted_view(ReachabilityViewKey(2), |a, b| b.cmp(a))
    ));
    assert_eq!(
        descending.ordered_co_values(&"V2"),
        vec![&"V5", &"V4", &"V3", &"V1"]
    );
    // Source's final line explicitly supplies the ascending comparator, despite
    // its comment mentioning null. Retain the actual call and identity assertion.
    assert!(Arc::ptr_eq(
        &ascending,
        &views.sorted_view(ReachabilityViewKey(1), Ord::cmp)
    ));
}
fn empty_change(values: Vec<i32>, entities: Vec<usize>) {
    let model = Arc::new(
        ListProblem::new(
            values.clone(),
            entities.iter().map(|_| ListEntity::new(vec![])).collect(),
        )
        .unwrap(),
    );
    let ctx = (model.clone(), model.initial_assignment());
    let mut source = ReplayedListValueChangeSource::new(
        Finite(values.clone()),
        Finite(entities),
        Finite(values),
        ListChangeDestinationOrder::Original,
        limits(),
        false,
        true,
    );
    let mut it = source.open(&ctx);
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}
#[test]
fn empty_change_value_selector() {
    empty_change(vec![], vec![0]);
}
#[test]
fn empty_change_entity_selector() {
    empty_change(vec![1], vec![]);
}
fn empty_swap(left: &[i32], right: &[i32]) {
    let model = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1, 2])]).unwrap();
    let layout = model.initial_assignment();
    let mut it = list_value_swap_moves(
        &model,
        &layout,
        left,
        right,
        ListSwapPairing::Cartesian,
        true,
    )
    .unwrap();
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}
#[test]
fn empty_swap_left_value_selector() {
    empty_swap(&[], &[1, 2]);
}
#[test]
fn empty_swap_right_value_selector() {
    empty_swap(&[1, 2], &[]);
}
