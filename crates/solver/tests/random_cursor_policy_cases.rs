// Adapted from pinned Community random iterator TESTs, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;

#[test]
fn repeating_empty_source() {
    let range = EnumeratedValueRange::<&str>::new(vec![]);
    let mut iterator = range.random_iter(0);
    assert!(iterator.by_ref().peekable().peek().is_none());
    assert_eq!(iterator.next(), None);
    assert_eq!(
        iterator.try_remove(),
        Err(RandomCursorOperationError::RemovalUnsupported)
    );
}
#[test]
fn repeating_remove_is_not_supported() {
    let range = EnumeratedValueRange::new(vec!["A"]);
    let mut iterator = range.random_iter(0);
    assert_eq!(iterator.next(), Some("A"));
    assert_eq!(
        iterator.try_remove(),
        Err(RandomCursorOperationError::RemovalUnsupported)
    );
}
#[test]
fn repeating_for_each_remaining_is_not_supported() {
    let range = EnumeratedValueRange::new(vec!["A"]);
    let mut iterator = range.random_iter(0);
    assert_eq!(
        iterator.try_for_each_remaining(|_| {}),
        Err(RandomCursorOperationError::RepeatingDrainUnsupported)
    );
}
#[test]
fn unique_remove_throws() {
    let values = ["A"];
    let mut iterator = unique_slice_samples(&values, 0);
    assert_eq!(iterator.next(), Some(Ok(&"A")));
    assert_eq!(
        iterator.try_remove(),
        Err(RandomCursorOperationError::RemovalUnsupported)
    );
}

fn assert_marker_types<R: RepeatingRandomCursor, U: UniqueRandomCursor>(
    mut repeating: R,
    mut unique: U,
) {
    // The generic bounds establish the actual factory-returned roles. Fallible
    // native operations replace exceptions; finite unique draining stays legal.
    assert_eq!(
        repeating.try_remove(),
        Err(RandomCursorOperationError::RemovalUnsupported)
    );
    assert_eq!(
        unique.try_remove(),
        Err(RandomCursorOperationError::RemovalUnsupported)
    );
    assert_eq!(
        repeating.try_for_each_remaining(|_| {}),
        Err(RandomCursorOperationError::RepeatingDrainUnsupported)
    );
}
#[test]
fn leaf_indexer_marker_types() {
    let mut index = CandidateIndex::new();
    index.insert(0, (), "Ann").unwrap();
    let bucket = index.bucket(&());
    assert_marker_types(bucket.repeating(0), bucket.unique(0));
}
#[test]
fn equal_indexer_marker_types() {
    let mut index = CandidateIndex::new();
    index.insert(0, "F", "Ann").unwrap();
    let bucket = index.bucket(&"F");
    assert_marker_types(bucket.repeating(0), bucket.unique(0));
}
