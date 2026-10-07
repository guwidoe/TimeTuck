// Adapted from pinned Containing/ContainedIn/ContainingAnyOfIndexerTest and
// AbstractIndexerTest helpers. Apache-2.0; see compat/provenance.json.
use timetuck::*;
type Index = MembershipCandidateIndex<(&'static str, &'static str), usize, usize>;
fn put(i: &mut Index, id: usize, skills: &[&'static str], partition: &'static str) {
    i.put(id, skills.iter().map(|&s| (s, partition)), id);
}
fn query<'a>(
    i: &'a Index,
    skills: &[&'static str],
    partition: &'static str,
) -> MembershipMatches<'a, usize, usize> {
    i.query_any(skills.iter().map(|&s| (s, partition)))
}
fn members(v: &MembershipMatches<'_, usize, usize>) -> Vec<usize> {
    v.iter().map(|v| v.id).collect()
}
fn drain(v: &MembershipMatches<'_, usize, usize>) {
    let expected = members(v);
    for seed in 0..10 {
        let mut it = v.unique(seed).peekable();
        let mut out = vec![];
        while it.peek().is_some() {
            out.push(it.next().unwrap().id);
        }
        out.sort();
        assert_eq!(out, expected);
        assert!(it.next().is_none());
        assert!(it.next().is_none());
    }
}
fn repeat(v: &MembershipMatches<'_, usize, usize>) {
    let expected = members(v);
    let mut it = v.repeating(0).peekable();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..30 {
        assert!(it.peek().is_some());
        let id = it.next().unwrap().id;
        assert!(expected.contains(&id));
        seen.insert(id);
    }
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), expected);
}
fn empty_retained() {
    let mut i = Index::new();
    assert!(i.is_empty());
    put(&mut i, 0, &[], "1");
    assert!(!i.is_empty());
}
#[test]
fn containing_is_removable() {
    empty_retained();
}
#[test]
fn any_is_removable() {
    empty_retained();
}
#[test]
fn contained_is_removable() {
    assert!(Index::new().is_empty());
}
fn remove_twice(skills: &[&'static str]) {
    let mut i = Index::new();
    put(&mut i, 0, skills, "1");
    i.remove(&0).unwrap();
    assert!(matches!(
        i.remove(&0),
        Err(CandidateIndexError::UnknownId(0))
    ));
}
#[test]
fn containing_remove_twice() {
    remove_twice(&["X", "Y"]);
}
#[test]
fn any_remove_twice() {
    remove_twice(&["X", "Y"]);
}
#[test]
fn contained_remove_twice() {
    remove_twice(&["X"]);
}
#[test]
fn any_for_each_duplicates() {
    let mut i = Index::new();
    put(&mut i, 0, &["X"], "");
    put(&mut i, 0, &["X"], "");
    put(&mut i, 1, &["X"], "");
    assert_eq!(members(&query(&i, &["X", "Y"], "")), [0, 1]);
}
#[test]
fn containing_for_each_contained_in_combo() {
    let mut i = Index::new();
    put(&mut i, 0, &["X", "Y"], "1");
    put(&mut i, 1, &["X", "Y"], "2");
    put(&mut i, 2, &["Y", "Z"], "1");
    assert_eq!(members(&i.query_any([("X", "1")])), [0]);
    assert_eq!(members(&i.query_any([("X", "1"), ("X", "2")])), [0, 1]);
    assert_eq!(members(&i.query_any([("Y", "1"), ("Y", "2")])), [0, 1, 2]);
    assert!(i.query_any([]).is_empty());
}
#[test]
fn contained_for_each_contain_combo() {
    let mut i = Index::new();
    i.put(0, [("X", "1"), ("X", "2")], 0);
    i.put(1, [("Y", "1"), ("Y", "2")], 1);
    i.put(2, [("X", "2"), ("X", "3")], 2);
    assert_eq!(members(&query(&i, &["X"], "1")), [0]);
    assert_eq!(members(&query(&i, &["X", "Y"], "1")), [0, 1]);
    assert_eq!(members(&query(&i, &["X", "Y"], "2")), [0, 1, 2]);
    assert!(query(&i, &[], "1").is_empty());
}
fn skills_index() -> Index {
    let mut i = Index::new();
    for (id, skills) in [
        (0, vec!["X", "Y"]),
        (1, vec!["X", "Z"]),
        (2, vec!["X", "Y"]),
        (3, vec![]),
    ] {
        put(&mut i, id, &skills, "");
    }
    i
}
fn distinct_seeds(v: &MembershipMatches<'_, usize, usize>) {
    let a: Vec<_> = v.unique(0).map(|v| v.id).collect();
    let b: Vec<_> = v.unique(2).map(|v| v.id).collect();
    let mut sorted_a = a.clone();
    let mut sorted_b = b.clone();
    sorted_a.sort();
    sorted_b.sort();
    assert_eq!(sorted_a, sorted_b);
    assert_ne!(a, b);
}
fn unique_skills(with_drain: bool) {
    let i = skills_index();
    for (keys, expected) in [
        (vec!["X"], vec![0, 1, 2]),
        (vec!["Y"], vec![0, 2]),
        (vec!["Z"], vec![1]),
    ] {
        let mut actual: Vec<_> = query(&i, &keys, "").unique(0).map(|v| v.id).collect();
        actual.sort();
        assert_eq!(actual, expected);
    }
    let v = query(&i, &["X"], "");
    distinct_seeds(&v);
    if with_drain {
        drain(&v);
    }
}
#[test]
fn containing_unique_random_iterator() {
    unique_skills(true);
}
#[test]
fn any_unique_random_iterator() {
    unique_skills(false);
}
#[test]
fn contained_unique_random_iterator() {
    let mut i = Index::new();
    for (id, skill) in ["X", "Y", "X", "Z", "X", "Y"].into_iter().enumerate() {
        put(&mut i, id, &[skill], "");
    }
    for (key, expected) in [("X", vec![0, 2, 4]), ("Y", vec![1, 5]), ("Z", vec![3])] {
        let mut actual: Vec<_> = query(&i, &[key], "").unique(0).map(|v| v.id).collect();
        actual.sort();
        assert_eq!(actual, expected);
    }
    distinct_seeds(&query(&i, &["X"], ""));
}
#[test]
fn any_unique_random_iterator_dedupes_across_buckets() {
    let mut i = Index::new();
    put(&mut i, 0, &["X", "Y"], "");
    put(&mut i, 1, &["X", "Z"], "");
    put(&mut i, 2, &["Y"], "");
    put(&mut i, 3, &[], "");
    drain(&query(&i, &["X", "Y", "Z"], ""));
}
#[test]
fn any_unique_random_iterator_single_draw_needs_no_dedupe() {
    let mut i = Index::new();
    put(&mut i, 0, &["X", "Y"], "");
    put(&mut i, 1, &["X", "Z"], "");
    put(&mut i, 2, &["Y"], "");
    let v = query(&i, &["X", "Y", "Z"], "");
    let mut it = v.unique(0).peekable();
    assert!(it.peek().is_some());
    assert!([0, 1, 2].contains(&it.next().unwrap().id));
}
#[test]
fn any_unique_random_iterator_duplicate_query_keys() {
    let mut i = Index::new();
    put(&mut i, 0, &["X", "Y"], "");
    put(&mut i, 1, &["X", "Z"], "");
    drain(&query(&i, &["X", "X"], ""));
}
#[test]
fn any_unique_random_iterator_unmatched_query_key() {
    let mut i = Index::new();
    put(&mut i, 0, &["X", "Y"], "");
    drain(&query(&i, &["X", "Q"], ""));
    assert!(query(&i, &["Q"], "").unique(0).next().is_none());
    assert!(query(&Index::new(), &["X"], "").unique(0).next().is_none());
}
#[test]
fn any_unique_random_iterator_composite_joiner() {
    let mut i = Index::new();
    put(&mut i, 0, &["X", "Y"], "1");
    put(&mut i, 1, &["X", "Z"], "1");
    put(&mut i, 2, &["X", "Y"], "2");
    let v = query(&i, &["X", "Y"], "1");
    assert_eq!(members(&v), [0, 1]);
    drain(&v);
}
#[test]
fn any_random_iterator_never_ends() {
    let mut i = Index::new();
    put(&mut i, 0, &["X", "Y"], "");
    put(&mut i, 1, &["X", "Z"], "");
    put(&mut i, 2, &["Y"], "");
    repeat(&query(&i, &["X", "Y", "Z"], ""));
    assert!(query(&i, &["Q"], "").repeating(0).next().is_none());
}
#[test]
fn containing_unique_random_iterator_unmatched_query_key() {
    let mut i = Index::new();
    put(&mut i, 0, &["X", "Y"], "");
    assert!(query(&i, &["Q"], "").unique(0).next().is_none());
}
#[test]
fn containing_random_iterator_never_ends() {
    let mut i = skills_index();
    i.remove(&3).unwrap();
    repeat(&query(&i, &["X"], ""));
    assert!(query(&i, &["Q"], "").repeating(0).next().is_none());
}
#[test]
fn contained_unique_random_iterator_drains_each_match_exactly_once() {
    let mut i = Index::new();
    for (id, key) in ["X", "Y", "X", "Z"].into_iter().enumerate() {
        put(&mut i, id, &[key], "");
    }
    drain(&query(&i, &["X", "Y", "Z"], ""));
}
#[test]
fn contained_duplicate_query_key_is_visited_once() {
    let mut i = Index::new();
    for (id, key) in ["X", "X", "Y"].into_iter().enumerate() {
        put(&mut i, id, &[key], "");
    }
    let v = query(&i, &["X", "X", "Y"], "");
    assert_eq!(v.len(), 3);
    assert_eq!(members(&v), [0, 1, 2]);
    drain(&v);
}
#[test]
fn contained_unique_random_iterator_unmatched_query_key() {
    let mut i = Index::new();
    put(&mut i, 0, &["X"], "");
    drain(&query(&i, &["X", "Q"], ""));
    assert!(query(&i, &["Q"], "").unique(0).next().is_none());
}
#[test]
fn contained_random_iterator_never_ends() {
    let mut i = Index::new();
    for (id, key) in ["X", "Y", "X"].into_iter().enumerate() {
        put(&mut i, id, &[key], "");
    }
    repeat(&query(&i, &["X", "Y"], ""));
    assert!(query(&i, &["Q"], "").repeating(0).next().is_none());
}
#[test]
fn contained_unique_random_iterator_first_draw_is_not_biased_to_first_bucket() {
    let mut i = Index::new();
    put(&mut i, 0, &["X"], "");
    put(&mut i, 1, &["Y"], "");
    let v = query(&i, &["X", "Y"], "");
    let mut seen = std::collections::BTreeSet::new();
    for seed in unique_indices(u64::MAX as u128, 0).take(20) {
        seen.insert(v.unique(seed.unwrap() as u64).next().unwrap().id);
    }
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), [0, 1]);
}
