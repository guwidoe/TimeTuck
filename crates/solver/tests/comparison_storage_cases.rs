// Adapted pinned ComparisonIndexerTest and AbstractIndexerTest helpers.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{collections::BTreeSet, sync::Arc};
use timetuck::*;
type Index = CandidateIndex<i32, usize, Arc<str>>;
fn fill(last: i32) -> Index {
    let mut i = Index::new();
    for age in 0..=last {
        i.insert(age as usize, age, Arc::from(format!("age{age}")))
            .unwrap();
    }
    i
}
fn churn(index: &mut Index) {
    // Native policy promotes on the next absent-key insert after the configured
    // number of removals; one extra transient cycle reaches that insertion.
    for attempt in 0..=index.storage_config().churn_tolerance {
        let id = usize::MAX - attempt;
        index.insert(id, -1, Arc::from("churn-1")).unwrap();
        index.remove(&id).unwrap();
    }
}
fn query(
    i: &Index,
    key: i32,
    op: JoinComparison,
) -> LazyConcat<'_, IndexedCandidate<usize, Arc<str>>> {
    comparison_candidates(i, key, op, CandidateSide::Left)
}
fn matches(v: &LazyConcat<'_, IndexedCandidate<usize, Arc<str>>>) -> Vec<usize> {
    v.iter().map(|v| v.unwrap().id).collect()
}
fn drain(v: &LazyConcat<'_, IndexedCandidate<usize, Arc<str>>>) {
    let expected: BTreeSet<_> = matches(v).into_iter().collect();
    for seed in 0..10 {
        let mut it = v.samples(seed).peekable();
        let mut out = vec![];
        while it.peek().is_some() {
            out.push(it.next().unwrap().unwrap().id);
        }
        assert_eq!(out.len(), expected.len());
        assert_eq!(out.into_iter().collect::<BTreeSet<_>>(), expected);
        assert!(it.next().is_none());
        assert!(it.next().is_none());
    }
}
#[test]
fn treeifies_past_array_threshold() {
    let threshold = Index::new().storage_config().array_threshold as i32;
    let mut i = fill(threshold);
    churn(&mut i);
    for age in threshold + 1..=threshold + 3 {
        i.insert(age as usize, age, Arc::from(format!("age{age}")))
            .unwrap();
    }
    assert!(!i.is_array_based());
    let all = query(&i, threshold + 10, JoinComparison::LessThan);
    assert_eq!(
        matches(&all),
        (0..=threshold + 3).map(|v| v as usize).collect::<Vec<_>>()
    );
    assert_eq!(all.len(), (threshold + 4) as usize);
    for row in all.iter() {
        let row = row.unwrap();
        assert_eq!(row.value.as_ref(), format!("age{}", row.id));
    }
    let below = query(&i, threshold / 2, JoinComparison::LessThan);
    assert_eq!(
        matches(&below),
        (0..threshold / 2).map(|v| v as usize).collect::<Vec<_>>()
    );
    assert_eq!(below.len(), (threshold / 2) as usize);
}
#[test]
fn treeify_is_one_way_no_demotion_on_remove() {
    let threshold = Index::new().storage_config().array_threshold as i32;
    let mut i = fill(threshold);
    churn(&mut i);
    assert!(!i.is_array_based());
    for age in 1..=threshold {
        i.remove(&(age as usize)).unwrap();
    }
    assert_eq!(query(&i, threshold + 10, JoinComparison::LessThan).len(), 1);
    assert!(!i.is_array_based());
    assert_eq!(i.get(&0).unwrap().value.as_ref(), "age0");
}
#[test]
fn monotonic_growth_stays_array_based() {
    let count = Index::new().storage_config().array_threshold * 4;
    let less = fill(count as i32 - 1);
    let greater = fill(count as i32 - 1);
    assert!(less.is_array_based());
    assert!(greater.is_array_based());
    let mid = count as i32 / 2;
    assert_eq!(
        matches(&query(&less, mid, JoinComparison::LessThan)),
        (0..mid).map(|v| v as usize).collect::<Vec<_>>()
    );
    assert_eq!(
        matches(&query(&greater, mid, JoinComparison::GreaterThan)),
        (mid + 1..count as i32)
            .map(|v| v as usize)
            .collect::<Vec<_>>()
    );
}
#[test]
fn unique_random_iterator_tree_mode() {
    let threshold = Index::new().storage_config().array_threshold as i32;
    let mut i = fill(threshold);
    churn(&mut i);
    assert!(!i.is_array_based());
    drain(&query(&i, threshold + 10, JoinComparison::LessThan));
}
#[test]
fn random_iterator_reverse_order_tree_mode() {
    let threshold = Index::new().storage_config().array_threshold as i32;
    let mut i = fill(threshold);
    churn(&mut i);
    assert!(!i.is_array_based());
    let v = query(&i, threshold - 2, JoinComparison::GreaterThan);
    let expected: BTreeSet<_> = matches(&v).into_iter().collect();
    assert_eq!(
        expected,
        [threshold as usize - 1, threshold as usize]
            .into_iter()
            .collect()
    );
    let mut seen = BTreeSet::new();
    let mut random = repeating_candidates(&v, 0).peekable();
    for _ in 0..60 {
        assert!(random.peek().is_some());
        let id = random.next().unwrap().unwrap().id;
        assert!(expected.contains(&id));
        seen.insert(id);
    }
    assert_eq!(seen, expected);
    drain(&v);
}
