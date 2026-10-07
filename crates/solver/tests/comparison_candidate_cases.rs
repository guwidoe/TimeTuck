// Adapted from pinned ComparisonIndexerTest and AbstractIndexerTest helpers.
// Apache-2.0; see compat/provenance.json. No Java storage-mode assertions adapted.
use timetuck::*;
type Index = CandidateIndex<i32, usize, usize>;
fn setup(keys: &[i32]) -> Index {
    let mut i = Index::new();
    for (id, &key) in keys.iter().enumerate() {
        i.insert(id, key, id).unwrap();
    }
    i
}
fn query(
    i: &Index,
    key: i32,
    op: JoinComparison,
) -> LazyConcat<'_, IndexedCandidate<usize, usize>> {
    comparison_candidates(i, key, op, CandidateSide::Left)
}
fn ids(v: &LazyConcat<'_, IndexedCandidate<usize, usize>>) -> Vec<usize> {
    v.iter().map(|v| v.unwrap().id).collect()
}
fn drain(v: &LazyConcat<'_, IndexedCandidate<usize, usize>>) {
    let mut expected = ids(v);
    expected.sort();
    for seed in 0..10 {
        let mut it = v.samples(seed).peekable();
        let mut out = vec![];
        while it.peek().is_some() {
            out.push(it.next().unwrap().unwrap().id);
        }
        out.sort();
        assert_eq!(out, expected);
        assert!(it.next().is_none());
        assert!(it.next().is_none());
    }
}
fn repeat(v: &LazyConcat<'_, IndexedCandidate<usize, usize>>, count: usize) {
    let mut expected = ids(v);
    expected.sort();
    let mut seen = std::collections::BTreeSet::new();
    let mut it = repeating_candidates(v, 0).peekable();
    for _ in 0..count {
        assert!(it.peek().is_some());
        let id = it.next().unwrap().unwrap().id;
        assert!(expected.contains(&id));
        seen.insert(id);
    }
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), expected);
}
#[test]
fn put_remove_size() {
    let mut i = Index::new();
    assert!(i.is_empty());
    i.insert(0, 40, 0).unwrap();
    i.insert(1, 50, 1).unwrap();
    assert!(!i.is_empty());
    assert_eq!(ids(&query(&i, 60, JoinComparison::LessThan)), [0, 1]);
    i.remove(&0).unwrap();
    assert_eq!(ids(&query(&i, 60, JoinComparison::LessThan)), [1]);
    assert_eq!(query(&i, 60, JoinComparison::LessThan).len(), 1);
    i.remove(&1).unwrap();
    assert!(i.is_empty());
    assert!(query(&i, 60, JoinComparison::LessThan).is_empty());
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct ExtremeKey(i32);
impl Ord for ExtremeKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let raw = if self.0 == other.0 {
            0
        } else if self.0 < other.0 {
            i32::MIN
        } else {
            i32::MAX
        };
        raw.cmp(&0)
    }
}
impl PartialOrd for ExtremeKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
#[test]
fn boundary_comparison_handles_extreme_compare_to_without_overflow() {
    let mut i = CandidateIndex::new();
    i.insert(0, ExtremeKey(1), "low").unwrap();
    for q in [2, 1] {
        let v = comparison_candidates(
            &i,
            ExtremeKey(q),
            JoinComparison::GreaterThan,
            CandidateSide::Left,
        );
        assert!(v.iter().next().is_none());
        assert_eq!(v.len(), 0);
    }
}
#[test]
fn unique_random_iterator_drains_each_match_exactly_once() {
    let i = setup(&[10, 10, 20, 30]);
    drain(&query(&i, 40, JoinComparison::LessThan));
}
#[test]
fn unique_random_iterator_includes_boundary_bucket() {
    let i = setup(&[10, 20, 20, 30]);
    let v = query(&i, 20, JoinComparison::LessThanOrEqual);
    assert_eq!(ids(&v), [0, 1, 2]);
    drain(&v);
}
#[test]
fn unique_random_iterator_single_bucket_map() {
    let i = setup(&[10, 10]);
    drain(&query(&i, 20, JoinComparison::LessThan));
    let v = query(&i, 5, JoinComparison::LessThan);
    let mut it = v.samples(0).peekable();
    assert!(it.peek().is_none());
    assert!(it.next().is_none());
}
#[test]
fn unique_random_iterator_empty_range() {
    let i = setup(&[10, 20]);
    let v = query(&i, 5, JoinComparison::LessThan);
    let mut it = v.samples(0).peekable();
    assert!(it.peek().is_none());
    assert!(it.next().is_none());
}
#[test]
fn random_iterator_empty_range() {
    let i = setup(&[10, 20]);
    assert!(
        repeating_candidates(&query(&i, 5, JoinComparison::LessThan), 0)
            .next()
            .is_none()
    );
}
#[test]
fn unique_random_iterator_first_draw_is_not_biased_to_first_bucket() {
    let i = setup(&[10, 20]);
    let v = query(&i, 30, JoinComparison::LessThan);
    let mut seen = std::collections::BTreeSet::new();
    for seed in unique_indices(u64::MAX as u128, 0).take(20) {
        seen.insert(v.samples(seed.unwrap() as u64).next().unwrap().unwrap().id);
    }
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), [0, 1]);
}
#[test]
fn random_iterator_reverse_order_array_mode() {
    let i = setup(&[10, 20, 20, 30]);
    let v = query(&i, 25, JoinComparison::GreaterThan);
    // Source comment says 20 and 30; actual GREATER_THAN(25) assertions use only 30.
    assert_eq!(ids(&v), [3]);
    repeat(&v, 100);
    drain(&v);
    let v = query(&i, 20, JoinComparison::GreaterThanOrEqual);
    assert_eq!(ids(&v), [1, 2, 3]);
    repeat(&v, 100);
    drain(&v);
}
#[test]
fn random_iterator_right_bridge_flip_reaches_reverse_walk() {
    let i = setup(&[10, 20, 20, 30]);
    let v = comparison_candidates(&i, 15, JoinComparison::LessThan, CandidateSide::Right);
    assert_eq!(ids(&v), [1, 2, 3]);
    repeat(&v, 100);
    drain(&v);
}
#[test]
fn random_iterator_single_bucket_map() {
    let i = setup(&[10, 10]);
    repeat(&query(&i, 20, JoinComparison::LessThan), 30);
    assert!(
        repeating_candidates(&query(&i, 5, JoinComparison::LessThan), 0)
            .next()
            .is_none()
    );
}
