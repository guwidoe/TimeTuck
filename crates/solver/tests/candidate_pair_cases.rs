// Adapted from pinned Community PairFairnessBiasIT and FilteredUniqueIterationTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::collections::{BTreeMap, BTreeSet};
use timetuck::*;
type Index = CandidateIndex<usize, usize, usize>;
fn fixture(sizes: &[usize]) -> (Index, Index) {
    let mut left = Index::new();
    let mut right = Index::new();
    let mut id = 0;
    for (bucket, &size) in sizes.iter().enumerate() {
        left.insert(bucket, bucket, bucket).unwrap();
        for _ in 0..size {
            right.insert(id, bucket, id).unwrap();
            id += 1;
        }
    }
    (left, right)
}
fn expected(sizes: &[usize]) -> BTreeSet<(usize, usize)> {
    let mut out = BTreeSet::new();
    let mut id = 0;
    for (bucket, &size) in sizes.iter().enumerate() {
        for _ in 0..size {
            out.insert((bucket, id));
            id += 1;
        }
    }
    out
}
fn uniform(
    label: &str,
    counts: &BTreeMap<(usize, usize), usize>,
    expected: &BTreeSet<(usize, usize)>,
    draws: usize,
) {
    assert_eq!(counts.keys().copied().collect::<BTreeSet<_>>(), *expected);
    assert_eq!(counts.values().sum::<usize>(), draws);
    let p = 1.0 / expected.len() as f64;
    let mean = draws as f64 * p;
    let deviation = (draws as f64 * p * (1.0 - p)).sqrt();
    let mut worst = 0.0_f64;
    for pair in expected {
        let z = (*counts.get(pair).unwrap() as f64 - mean).abs() / deviation;
        worst = worst.max(z);
    }
    eprintln!(
        "{label}: {draws} draws, {} pairs, worst {worst:.4} sigma (limit 5)",
        expected.len()
    );
    assert!(worst <= 5.0, "{label}: worst deviation {worst}");
}
#[test]
fn indexed_join_buckets_are_uniform_per_pair() {
    for sizes in [vec![2, 20], vec![10, 20, 40, 80, 160]] {
        let (left, right) = fixture(&sizes);
        let pairs = left.join(&right, JoinComparison::Equal).unwrap();
        let mut provider = PairMoveProvider::new(
            pairs,
            PairSampling::Repeating,
            0,
            620_000,
            |_: &(),
             _: &(),
             _: u64,
             l: &IndexedCandidate<usize, usize>,
             r: &IndexedCandidate<usize, usize>| Some((l.id, r.id)),
        );
        let mut counts = BTreeMap::new();
        for pair in provider.moves(&(), &(), 0) {
            *counts.entry(pair).or_default() += 1;
        }
        uniform("indexed pair provider", &counts, &expected(&sizes), 620_000);
    }
}
#[test]
fn cached_join_same_skewed_shape_stays_uniform_over_all_materialized_pairs() {
    let sizes = [10, 20, 40, 80, 160];
    let (left, right) = fixture(&sizes);
    let pairs = left.join(&right, JoinComparison::Equal).unwrap();
    let mut cached = CandidateIndex::new();
    for (id, (l, r)) in pairs.iter().enumerate() {
        cached.insert(id, (), (l.id, r.id)).unwrap();
    }
    let mut counts = BTreeMap::new();
    for entry in cached.bucket(&()).repeating(0).take(620_000) {
        *counts.entry(entry.value).or_default() += 1;
    }
    uniform(
        "materialized pair cache",
        &counts,
        &expected(&sizes),
        620_000,
    );
}
#[test]
fn just_in_time_join_unique_drain_covers_every_pair_exactly_once() {
    let sizes = [10, 20, 40, 80, 160];
    let (left, right) = fixture(&sizes);
    let pairs = left.join(&right, JoinComparison::Equal).unwrap();
    let mut seen = BTreeSet::new();
    for (l, r) in pairs.unique(0) {
        assert!(seen.insert((l.id, r.id)));
    }
    assert_eq!(seen.len(), sizes.iter().sum::<usize>());
    assert_eq!(seen, expected(&sizes));
}
#[test]
fn throws_when_unknowingly_empty() {
    for seed in 0..10 {
        for count in 1..=4 {
            let mut index = CandidateIndex::new();
            let elements = ["A", "B", "C", "D"];
            for (i, value) in elements[..count].iter().enumerate() {
                index.insert(i, (), *value).unwrap();
            }
            let mut it = index
                .bucket(&())
                .unique(seed)
                .filter(|c| !elements[..count].contains(&c.value));
            assert!(it.next().is_none());
            assert!(it.next().is_none());
        }
    }
}
