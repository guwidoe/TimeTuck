// Original native role-policy controls; no upstream method credit.
use std::cell::Cell;
use timetuck::*;

#[test]
fn rejected_operations_preserve_draws_and_never_call_consumers() {
    let values = (0..65).collect::<Vec<_>>();
    let range = EnumeratedValueRange::new(values.clone());
    let calls = Cell::new(0);
    let mut repeating = range.random_iter_with(|bound| {
        calls.set(calls.get() + 1);
        7 % bound
    });
    for _ in 0..20 {
        assert_eq!(
            repeating.try_remove(),
            Err(RandomCursorOperationError::RemovalUnsupported)
        );
        assert_eq!(
            repeating.try_for_each_remaining(|_| panic!("must not drain")),
            Err(RandomCursorOperationError::RepeatingDrainUnsupported)
        );
    }
    assert_eq!(calls.get(), 0);
    assert_eq!(repeating.next(), Some(7));
    assert_eq!(calls.get(), 1);
    let mut unique = unique_slice_samples(&values, 123);
    let mut reference = unique_slice_samples(&values, 123);
    for _ in 0..values.len() {
        assert_eq!(
            unique.try_remove(),
            Err(RandomCursorOperationError::RemovalUnsupported)
        );
        assert_eq!(unique.next(), reference.next());
    }
    assert_eq!(unique.next(), None);
    assert_eq!(
        unique.try_remove(),
        Err(RandomCursorOperationError::RemovalUnsupported)
    );
}

#[test]
fn lazy_query_roles_do_not_load_groups_or_clone_payloads_on_rejection() {
    struct Payload(usize);
    let calls = Cell::new(0);
    let mut query = LazyConcat::new();
    query
        .push_deferred(65, || {
            calls.set(calls.get() + 1);
            (0..65).map(Payload).collect()
        })
        .unwrap();
    let mut repeating = repeating_candidates(&query, 0);
    let mut unique = query.samples(0);
    assert!(repeating.try_remove().is_err());
    assert!(unique.try_remove().is_err());
    assert!(
        repeating
            .try_for_each_remaining(|_| panic!("rejected"))
            .is_err()
    );
    assert_eq!(calls.get(), 0);
    assert!(repeating.next().unwrap().unwrap().0 < 65);
    let mut values: Vec<_> = unique.by_ref().map(|v| v.unwrap().0).collect();
    values.sort_unstable();
    assert_eq!(values, (0..65).collect::<Vec<_>>());
    assert_eq!(calls.get(), 1);
    unique
        .by_ref()
        .for_each(|_| panic!("exhausted unique cursor"));
    assert_eq!(calls.get(), 1);
}

#[test]
fn empty_views_reject_bulk_drain_and_unique_range_stays_finite() {
    let index = CandidateIndex::<(), usize, usize>::new();
    let bucket = index.bucket(&());
    let mut repeated = bucket.repeating(0);
    assert!(
        repeated
            .try_for_each_remaining(|_| panic!("empty"))
            .is_err()
    );
    assert_eq!(repeated.next(), None);
    let mut unique = bucket.unique(0);
    assert!(unique.try_remove().is_err());
    assert_eq!(unique.next(), None);
    let range = EnumeratedValueRange::new(vec![1, 1, 2]);
    let mut cursor = unique_range_samples(&range, 0);
    assert!(cursor.try_remove().is_err());
    let mut output = vec![];
    cursor
        .by_ref()
        .for_each(|value| output.push(value.unwrap()));
    output.sort();
    assert_eq!(output, [1, 1, 2]);
    assert_eq!(cursor.next(), None);
}
