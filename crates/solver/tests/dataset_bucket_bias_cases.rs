// Adapted from pinned Community DatasetBucketBiasIT and BiasReport helpers.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::num::NonZeroUsize;
use timetuck::*;
#[test]
fn buckets_weight_remaining_members_at_each_requested_draw() {
    let mut left = CandidateIndex::new();
    left.insert(0, 50, ()).unwrap();
    let mut right = CandidateIndex::new();
    let mut id = 0;
    for (bucket, (key, size)) in [(10, 2), (20, 3), (30, 5)].into_iter().enumerate() {
        for _ in 0..size {
            right.insert(id, key, bucket).unwrap();
            id += 1;
        }
    }
    let pairs = left
        .join(&right, JoinComparison::GreaterThanOrEqual)
        .unwrap();
    for at in [1, 2, 5] {
        let mut root = SampleRandom::new(0);
        let mut counts = [0; 3];
        for _ in 0..200_000 {
            let seed = root.index(NonZeroUsize::new(usize::MAX).unwrap()) as u64;
            let mut draws = pairs.retiring(seed);
            let mut picked = None;
            for _ in 0..at {
                picked = Some(draws.next().unwrap().1.value);
                draws.retire().unwrap();
            }
            counts[picked.unwrap()] += 1;
        }
        assert_eq!(counts.iter().sum::<usize>(), 200_000);
        for (count, p) in counts.into_iter().zip([0.2_f64, 0.3, 0.5]) {
            assert!(count > 0);
            let sigma = (count as f64 - 200_000.0 * p).abs() / (200_000.0 * p * (1.0 - p)).sqrt();
            assert!(
                sigma <= 5.0,
                "draw {at}, observed {count}, p {p}, sigma {sigma}"
            );
        }
    }
}
