// Adapted from pinned EqualsAndComparisonIndexerTest and AbstractIndexerTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
type Index = CandidateIndex<(&'static str, u8), &'static str, &'static str>;
fn setup(names: &[&'static str]) -> Index {
    let mut i = Index::new();
    for &name in names {
        let mut parts = name.split('-');
        parts.next();
        let gender = parts.next().unwrap();
        let age = parts.next().unwrap().parse().unwrap();
        i.insert(name, (gender, age), name).unwrap();
    }
    i
}
fn view(i: &Index) -> LazyConcat<'_, IndexedCandidate<&'static str, &'static str>> {
    i.range(("F", 0)..=("F", 40))
}
#[test]
fn remove_twice() {
    let mut i = setup(&["Ann-F-40"]);
    i.remove(&"Ann-F-40").unwrap();
    assert!(matches!(
        i.remove(&"Ann-F-40"),
        Err(CandidateIndexError::UnknownId(_))
    ));
}
#[test]
fn unique_random_iterator_empty() {
    let i = Index::new();
    let v = view(&i);
    let mut it = v.samples(0).peekable();
    assert!(it.peek().is_none());
    assert!(it.next().is_none());
}
#[test]
fn unique_random_iterator_single_element() {
    let i = setup(&["Ann-F-40"]);
    let v = view(&i);
    let mut it = v.samples(0).peekable();
    assert!(it.peek().is_some());
    assert_eq!(it.next().unwrap().unwrap().value, "Ann-F-40");
    assert!(it.next().is_none());
}
#[test]
fn unique_random_iterator_multiple_elements() {
    let i = setup(&["Ann-F-40", "Beth-F-30"]);
    let v = view(&i);
    let mut it = v.samples(0).peekable();
    assert!(it.peek().is_some());
    assert!(["Ann-F-40", "Beth-F-30"].contains(&it.next().unwrap().unwrap().value));
}
#[test]
fn unique_random_iterator_comparison_boundary() {
    let i = setup(&["Ann-F-40", "Beth-F-30", "Carol-F-50"]);
    let v = view(&i);
    let mut out: Vec<_> = v.samples(0).map(|v| v.unwrap().value).collect();
    out.sort();
    assert_eq!(out, ["Ann-F-40", "Beth-F-30"]);
    assert!(!out.contains(&"Carol-F-50"));
}
#[test]
fn unique_random_iterator_remove_all_elements() {
    let expected = ["Ann-F-40", "Beth-F-30", "Carol-F-35"];
    let i = setup(&expected);
    let v = view(&i);
    let mut it = v.samples(0).peekable();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..3 {
        assert!(it.peek().is_some());
        seen.insert(it.next().unwrap().unwrap().value);
    }
    assert!(it.peek().is_none());
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), expected);
    assert!(it.next().is_none());
}
#[test]
fn unique_random_iterator_with_filter() {
    let i = setup(&["Ann-F-40", "Beth-F-30", "Carol-F-35"]);
    let v = view(&i);
    let mut out: Vec<_> = v
        .samples(0)
        .map(|v| v.unwrap().value)
        .filter(|v| v.starts_with('A') || v.starts_with('C'))
        .collect();
    out.sort();
    assert_eq!(out, ["Ann-F-40", "Carol-F-35"]);
    assert!(!out.contains(&"Beth-F-30"));
}
#[test]
fn unique_random_iterator_with_filter_empty() {
    let i = setup(&["Ann-F-40", "Beth-F-30"]);
    let v = view(&i);
    assert!(!v.samples(0).any(|_| false));
}
#[test]
fn unique_random_iterator_with_filter_and_boundary() {
    let i = setup(&["Ann-F-40", "Beth-F-30", "Carol-F-50"]);
    let v = view(&i);
    let out: Vec<_> = v
        .samples(0)
        .map(|v| v.unwrap().value)
        .filter(|v| v.starts_with('B'))
        .collect();
    assert_eq!(out, ["Beth-F-30"]);
    assert!(!out.contains(&"Ann-F-40"));
    assert!(!out.contains(&"Carol-F-50"));
}
#[test]
fn random_iterator_right_bridge_flip_reaches_reverse_walk() {
    let i = setup(&["Ann-F-40", "Beth-F-30", "Carol-F-50"]);
    let v = i.range(("F", 40)..=("F", u8::MAX));
    let mut expected: Vec<_> = v.iter().map(|r| r.unwrap().value).collect();
    expected.sort();
    assert_eq!(expected, ["Ann-F-40", "Carol-F-50"]);
    let mut it = repeating_candidates(&v, 0).peekable();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..60 {
        assert!(it.peek().is_some());
        let value = it.next().unwrap().unwrap().value;
        assert!(expected.contains(&value));
        seen.insert(value);
    }
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), expected);
    for seed in 0..10 {
        let mut it = v.samples(seed);
        let mut out: Vec<_> = it.by_ref().map(|r| r.unwrap().value).collect();
        out.sort();
        assert_eq!(out, expected);
        assert!(it.next().is_none());
        assert!(it.next().is_none());
    }
}
