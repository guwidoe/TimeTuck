// Adapted from pinned EqualIndexerTest and AbstractIndexerTest helper assertions.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
type Index = CandidateIndex<(&'static str, u8), &'static str, &'static str>;
fn add(i: &mut Index, name: &'static str, key: (&'static str, u8)) {
    i.insert(name, key, name).unwrap();
}
fn values(i: &Index, key: (&'static str, u8)) -> Vec<&'static str> {
    let mut out: Vec<_> = i.bucket(&key).iter().map(|v| v.value).collect();
    out.sort();
    out
}
#[test]
fn is_removable() {
    assert!(values(&Index::new(), ("F", 40)).is_empty());
}
#[test]
fn put() {
    let mut i = Index::new();
    assert_eq!(i.bucket(&("F", 40)).len(), 0);
    add(&mut i, "Ann-F-40", ("F", 40));
    assert_eq!(i.bucket(&("F", 40)).len(), 1);
}
#[test]
fn remove_twice() {
    let mut i = Index::new();
    add(&mut i, "Ann-F-40", ("F", 40));
    i.remove(&"Ann-F-40").unwrap();
    assert!(matches!(
        i.remove(&"Ann-F-40"),
        Err(CandidateIndexError::UnknownId(_))
    ));
}
#[test]
fn for_each() {
    let mut i = Index::new();
    for (n, k) in [
        ("Ann-F-40", ("F", 40)),
        ("Beth-F-30", ("F", 30)),
        ("Carl-M-40", ("M", 40)),
        ("Dan-M-30", ("M", 30)),
        ("Edna-F-40", ("F", 40)),
    ] {
        add(&mut i, n, k);
    }
    assert_eq!(values(&i, ("F", 40)), ["Ann-F-40", "Edna-F-40"]);
    assert_eq!(values(&i, ("F", 30)), ["Beth-F-30"]);
    assert!(values(&i, ("F", 20)).is_empty());
}
#[test]
fn iterator_empty() {
    let i = Index::new();
    assert!(i.bucket(&("F", 40)).iter().next().is_none());
}
#[test]
fn iterator_single_element() {
    let mut i = Index::new();
    add(&mut i, "Ann-F-40", ("F", 40));
    let mut it = i.bucket(&("F", 40)).iter().peekable();
    assert!(it.peek().is_some());
    assert_eq!(it.next().unwrap().value, "Ann-F-40");
    assert!(it.next().is_none());
}
#[test]
fn iterator_multiple_elements() {
    let mut i = Index::new();
    for (n, k) in [
        ("Ann-F-40", ("F", 40)),
        ("Beth-F-30", ("F", 30)),
        ("Edna-F-40", ("F", 40)),
    ] {
        add(&mut i, n, k);
    }
    assert_eq!(values(&i, ("F", 40)), ["Ann-F-40", "Edna-F-40"]);
}
#[test]
fn iterator_no_matching_key() {
    let mut i = Index::new();
    add(&mut i, "Ann-F-40", ("F", 40));
    add(&mut i, "Bob-M-30", ("M", 30));
    assert!(i.bucket(&("F", 20)).iter().next().is_none());
}
#[test]
fn unique_random_iterator_empty() {
    let i = Index::new();
    let mut it = i.bucket(&("F", 40)).unique(0).peekable();
    assert!(it.peek().is_none());
    assert!(it.next().is_none());
}
#[test]
fn unique_random_iterator_single_element() {
    let mut i = Index::new();
    add(&mut i, "Ann-F-40", ("F", 40));
    let mut it = i.bucket(&("F", 40)).unique(0).peekable();
    assert!(it.peek().is_some());
    assert_eq!(it.next().unwrap().value, "Ann-F-40");
    assert!(it.next().is_none());
}
#[test]
fn unique_random_iterator_multiple_elements() {
    let mut i = Index::new();
    add(&mut i, "Ann-F-40", ("F", 40));
    add(&mut i, "Edna-F-40", ("F", 40));
    add(&mut i, "Beth-F-30", ("F", 30));
    let mut it = i.bucket(&("F", 40)).unique(0).peekable();
    assert!(it.peek().is_some());
    assert!(["Ann-F-40", "Edna-F-40"].contains(&it.next().unwrap().value));
}
#[test]
fn unique_random_iterator_remove_all_elements() {
    let mut i = Index::new();
    let expected = ["Ann-F-40", "Edna-F-40", "France-F-40"];
    for n in expected {
        add(&mut i, n, ("F", 40));
    }
    let mut it = i.bucket(&("F", 40)).unique(0).peekable();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..3 {
        assert!(it.peek().is_some());
        seen.insert(it.next().unwrap().value);
    }
    assert!(it.peek().is_none());
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), expected);
    assert!(it.next().is_none());
}
#[test]
fn unique_random_iterator_with_filter() {
    let mut i = Index::new();
    for n in ["Ann-F-40", "Beth-F-40", "Carol-F-40"] {
        add(&mut i, n, ("F", 40));
    }
    let mut out: Vec<_> = i
        .bucket(&("F", 40))
        .unique(0)
        .filter(|e| e.value.starts_with('A') || e.value.starts_with('C'))
        .map(|e| e.value)
        .collect();
    out.sort();
    assert_eq!(out, ["Ann-F-40", "Carol-F-40"]);
    assert!(!out.contains(&"Beth-F-40"));
}
#[test]
fn random_iterator_never_ends() {
    let mut i = Index::new();
    let expected = ["Ann-F-40", "Edna-F-40"];
    for n in expected {
        add(&mut i, n, ("F", 40));
    }
    let mut it = i.bucket(&("F", 40)).repeating(0).peekable();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..30 {
        assert!(it.peek().is_some());
        let value = it.next().unwrap().value;
        assert!(expected.contains(&value));
        seen.insert(value);
    }
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), expected);
    assert!(i.bucket(&("F", 20)).repeating(0).next().is_none());
}
#[test]
fn unique_random_iterator_with_filter_empty() {
    let mut i = Index::new();
    for n in ["Ann-F-40", "Beth-F-40"] {
        add(&mut i, n, ("F", 40));
    }
    assert!(!i.bucket(&("F", 40)).unique(0).any(|_| false));
}
