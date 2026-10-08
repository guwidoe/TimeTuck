// Adapted from Timefold Community RandomAccessLeafIndexerTest and its shared
// repeating-iterator assertion, Apache-2.0. See compat/provenance.json.
use std::collections::BTreeSet;
use timetuck::{CandidateIndex, CandidateIndexError};
#[test]
fn is_removable() {
    let index = CandidateIndex::<(), usize, &str>::new();
    assert!(index.bucket(&()).iter().next().is_none());
    assert!(index.is_empty());
}
#[test]
fn put() {
    let mut index = CandidateIndex::new();
    assert_eq!(index.bucket(&()).len(), 0);
    index.insert(0, (), "Ann-F-40").unwrap();
    assert_eq!(index.bucket(&()).len(), 1);
    assert!(!index.is_empty());
    assert_eq!(
        index
            .bucket(&())
            .iter()
            .map(|r| r.value)
            .collect::<Vec<_>>(),
        ["Ann-F-40"]
    );
}
#[test]
fn remove_twice() {
    let mut index = CandidateIndex::new();
    index.insert(0, (), "Ann-F-40").unwrap();
    assert!(!index.is_empty());
    assert_eq!(
        index
            .bucket(&())
            .iter()
            .map(|r| r.value)
            .collect::<Vec<_>>(),
        ["Ann-F-40"]
    );
    index.remove(&0).unwrap();
    assert!(index.is_empty());
    assert!(index.bucket(&()).iter().next().is_none());
    assert_eq!(index.remove(&0), Err(CandidateIndexError::UnknownId(0)));
}
#[test]
fn for_each() {
    let mut index = CandidateIndex::new();
    index.insert(0, (), "Ann-F-40").unwrap();
    index.insert(1, (), "Beth-F-30").unwrap();
    assert_eq!(
        index
            .bucket(&())
            .iter()
            .map(|r| r.value)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["Ann-F-40", "Beth-F-30"])
    );
}
#[test]
fn random_iterator_never_ends() {
    let mut index = CandidateIndex::new();
    index.insert(0, (), "Ann-F-40").unwrap();
    index.insert(1, (), "Beth-F-30").unwrap();
    let bucket = index.bucket(&());
    let expected = bucket.iter().map(|r| r.value).collect::<BTreeSet<_>>();
    let mut iter = bucket.repeating(0).peekable();
    let mut drawn = BTreeSet::new();
    for _ in 0..30 {
        assert!(iter.peek().is_some());
        let row = iter.next().unwrap();
        assert!(expected.contains(row.value));
        drawn.insert(row.value);
    }
    assert_eq!(drawn, expected);
}
