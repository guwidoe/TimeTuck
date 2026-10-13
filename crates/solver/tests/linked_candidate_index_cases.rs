// Apache-2.0; full IndexerRandomIteratorMarkerTest source method adaptation.
use timetuck::*;
#[test]
fn linked_list_leaf_indexer_rejects_random_access() {
    let mut index = LinkedCandidateIndex::new();
    index.insert(0, (), "Ann").unwrap();
    let leaf = index.bucket(&());
    assert!(matches!(
        leaf.repeating(0),
        Err(LeafRandomAccessUnsupported)
    ));
    assert!(matches!(leaf.unique(0), Err(LeafRandomAccessUnsupported)));
}
