// Adapted from pinned Community LeftRetirementBiasIT, PairFairnessBiasIT,
// BucketedFixture, AbstractBiasIT and BiasReport. Apache-2.0; see provenance.
use timetuck::*;
fn buckets(sizes: &[usize]) -> Vec<(usize, usize)> {
    sizes
        .iter()
        .enumerate()
        .flat_map(|(bucket, &size)| (0..size).map(move |id| (bucket, id)))
        .collect()
}
fn sigma(counts: &[usize], trials: usize, probabilities: &[f64]) {
    assert_eq!(counts.iter().sum::<usize>(), trials);
    for (&count, &p) in counts.iter().zip(probabilities) {
        assert!(count > 0);
        let deviation =
            (count as f64 - trials as f64 * p).abs() / (trials as f64 * p * (1.0 - p)).sqrt();
        assert!(deviation <= 5.0, "count={count}, p={p}, sigma={deviation}");
    }
}
fn movement<A: Copy>((left, right): (&usize, &A)) -> Move<A> {
    Move::Change {
        variable: *left,
        value: Some(*right),
    }
}
#[test]
fn interior_retirements_leave_every_surviving_entity_uniform() {
    let left: Vec<_> = (0..20).collect();
    let dead = [4, 8, 12, 16];
    let sizes: Vec<_> = left
        .iter()
        .map(|i| usize::from(!dead.contains(i)))
        .collect();
    let right = buckets(&sizes);
    let mut pairs =
        FilteredPairSampler::new(&left, &right, |a, b| *a == b.0, 0, 1024).map(movement);
    for _ in 0..1000 {
        pairs.next().unwrap();
    }
    let mut counts = [0; 20];
    for _ in 0..200_000 {
        let m = pairs.next().unwrap();
        let entity = m.touched_variables()[0];
        assert!(!dead.contains(&entity));
        counts[entity] += 1;
    }
    let live: Vec<_> = counts
        .into_iter()
        .enumerate()
        .filter_map(|(i, n)| (!dead.contains(&i)).then_some(n))
        .collect();
    sigma(&live, 200_000, &[1.0 / 16.0; 16]);
}
#[test]
fn hard_to_match_entity_stays_reachable_throughout_the_run() {
    let left = [0, 1];
    let right: Vec<_> = (0..20).collect();
    let pairs = FilteredPairSampler::new(&left, &right, |a, b| *a == 1 || *b == 0, 0, 1024);
    let mut moves = pairs.map(movement);
    let mut counts = [0; 2];
    let mut hard_in_tail = 0;
    for draw in 0..400_000 {
        let m = moves.next().unwrap();
        let entity = m.touched_variables()[0];
        if draw >= 300_000 && entity == 0 {
            hard_in_tail += 1;
        }
        counts[entity] += 1;
    }
    assert!(hard_in_tail > 0);
    sigma(&counts, 400_000, &[0.5, 0.5]);
}
#[test]
fn filtering_only_join_preserves_small_bucket_per_pair_bias() {
    let sizes = [10, 20, 40, 80, 160];
    let left = [0, 1, 2, 3, 4];
    let right = buckets(&sizes);
    let mut moves =
        FilteredPairSampler::new(&left, &right, |a, b| *a == b.0, 0, 1024).map(movement);
    let mut counts = [0; 5];
    for _ in 0..620_000 {
        let Move::Change {
            variable,
            value: Some((bucket, member)),
        } = moves.next().unwrap()
        else {
            panic!("change move expected")
        };
        assert_eq!(variable, bucket);
        assert!(member < sizes[bucket]);
        counts[bucket] += 1;
    }
    assert!(counts[0] > 0 && counts[4] > 0);
    let small_rate = counts[0] as f64 / 10.0;
    let large_rate = counts[4] as f64 / 160.0;
    assert!(
        small_rate > large_rate * 5.0,
        "small={small_rate}, large={large_rate}"
    );
}
