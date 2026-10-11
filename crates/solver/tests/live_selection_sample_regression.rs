// Original regression tests; no upstream-method credit.
use std::{
    cell::Cell,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    ops::Deref,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
use timetuck::*;
fn hash(v: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}
#[derive(Debug, Eq, PartialEq, Hash)]
struct MoveOnly(i32);
#[test]
fn sample_view_retains_allocations_and_observes_storage_transitions() {
    let set = ScalingSequencedSet::new();
    let view = NeighborhoodSampleView::from_set(&set);
    for i in 0..16 {
        set.insert(MoveOnly(i)).unwrap();
    }
    let retained = view.representative().unwrap();
    let SampleMember::Shared(ref allocation) = retained else {
        panic!("set must share");
    };
    assert!(Arc::ptr_eq(allocation, &set.first().unwrap()));
    set.insert(MoveOnly(16)).unwrap();
    assert_eq!(view.len(), 17);
    set.remove(&MoveOnly(0)).unwrap();
    set.remove(&MoveOnly(1)).unwrap();
    assert_eq!(view.len(), 15);
    assert_eq!(retained.0, 0);
    assert_eq!(view.representative().unwrap().0, 2);
    let mut cursor = view.iter();
    assert_eq!(cursor.next().unwrap().0, 2);
    assert_eq!(set.insert(MoveOnly(19)), Err(SequencedSetBorrowed));
    drop(cursor);
    set.clear().unwrap();
    assert!(view.is_empty());
    assert!(view.representative().is_none());
    assert_eq!(retained.0, 0);
    let slice = [MoveOnly(7), MoveOnly(7), MoveOnly(8)];
    let borrowed = NeighborhoodSampleView::from_slice(&slice);
    let item = borrowed.representative().unwrap();
    assert!(std::ptr::eq(item.deref(), &slice[0]));
    assert_eq!(borrowed.len(), 2);
    assert_eq!(borrowed.iter().map(|v| v.0).collect::<Vec<_>>(), [7, 8]);
}
#[test]
fn view_equality_and_hash_follow_live_membership_and_nullable_duplicates() {
    let members = [Some("b"), None, Some("b"), Some("a"), None];
    let slice = NeighborhoodSampleView::from_slice(&members);
    let owned = NeighborhoodSample::new([None, Some("a"), Some("b")]);
    let set = ScalingSequencedSet::new();
    set.extend([Some("a"), None, Some("b")]).unwrap();
    let live = NeighborhoodSampleView::from_set(&set);
    assert!(slice == live && live == owned && owned == slice);
    assert_eq!(hash(&slice), hash(&live));
    assert_eq!(hash(&live), hash(&owned));
    set.remove(&None).unwrap();
    assert!(live != slice);
    assert_eq!(live.len(), 2);
    set.insert(None).unwrap();
    assert!(live == slice);
    assert!(NeighborhoodSampleView::<i32>::from_slice(&[]).is_empty());
}
#[test]
fn random_dataset_bulk_guard_is_passive_and_bounded_adapters_work() {
    let data = NeighborhoodDataset::new([1, 2, 3]);
    let mut a = SampleRandom::new(19);
    let mut b = SampleRandom::new(19);
    let mut it = data.random_iter(&mut a);
    let calls = Cell::new(0);
    for _ in 0..4 {
        assert!(it.has_next());
        assert_eq!(
            it.try_for_each_remaining(|_| calls.set(calls.get() + 1)),
            Err(UnboundedDatasetIteration)
        );
    }
    assert_eq!(it.next(), data.random_iter(&mut b).next());
    assert_eq!(calls.get(), 0);
    drop(it);
    let panic = catch_unwind(AssertUnwindSafe(|| {
        data.random_iter(&mut a)
            .for_each(|_| calls.set(calls.get() + 1))
    }))
    .unwrap_err();
    assert!(panic.is::<UnboundedDatasetIteration>());
    assert_eq!(calls.get(), 0);
    data.random_iter(&mut a)
        .take(7)
        .for_each(|_| calls.set(calls.get() + 1));
    assert_eq!(calls.get(), 7);
    let empty = NeighborhoodDataset::<i32>::default();
    let mut it = empty.random_iter(&mut a);
    assert!(!it.has_next());
    assert_eq!(it.try_for_each_remaining(|_| panic!("empty")), Ok(()));
    it.for_each(|_| panic!("empty"));
    assert_eq!(
        data.random_iter(&mut a).next(),
        data.random_iter(&mut b).nth(7)
    );
}
fn source(
    children: usize,
    ignore: bool,
    bounds: &[Option<u128>],
    calls: &Cell<usize>,
) -> Result<Option<u128>, StreamingCardinalityError> {
    let s = StreamingSelection::<(), i32, std::array::IntoIter<i32, 1>, _, _>::new(
        children,
        ignore,
        NonZeroUsize::new(2).unwrap(),
        |_: &(), _: usize, _: &DependentPrefix<i32>| {
            calls.set(calls.get() + 1);
            [1].into_iter()
        },
        |_: SelectionLifecycleEvent, _: &()| calls.set(calls.get() + 1),
    )
    .with_child_catalog_bounds(bounds.iter().copied())?;
    let result = s.with_termination(|| false).catalog_upper_bound();
    Ok(result)
}
#[test]
fn catalog_metadata_handles_empty_unknown_and_overflow_without_callbacks() {
    let calls = Cell::new(0);
    assert_eq!(source(2, false, &[Some(6), Some(4)], &calls), Ok(Some(24)));
    assert_eq!(source(2, false, &[None, Some(0)], &calls), Ok(Some(0)));
    assert_eq!(source(2, true, &[None, Some(0)], &calls), Ok(None));
    assert_eq!(source(2, true, &[Some(0), Some(4)], &calls), Ok(Some(4)));
    assert_eq!(source(2, true, &[Some(0), Some(0)], &calls), Ok(Some(0)));
    assert_eq!(source(0, true, &[], &calls), Ok(Some(0)));
    assert_eq!(
        source(2, false, &[Some(u128::MAX), Some(2)], &calls),
        Err(StreamingCardinalityError::Overflow)
    );
    assert_eq!(
        source(2, true, &[Some(1)], &calls),
        Err(StreamingCardinalityError::ChildCount {
            expected: 2,
            actual: 1
        })
    );
    assert_eq!(calls.get(), 0);
}
#[test]
fn positive_external_capacity_and_native_zero_capacity_remain_distinct() {
    let set = ScalingSequencedSet::try_with_expected_size(1).unwrap();
    set.extend(0..33).unwrap();
    assert_eq!(set.len(), 33);
    let native = ScalingSequencedSet::with_capacity(0);
    native.insert(7).unwrap();
    assert_eq!(native.len(), 1);
}
