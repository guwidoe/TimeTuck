// Adapted from pinned Community SamplingIteratorBiasIT and BiasReport.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::num::NonZeroUsize;
use timetuck::*;
fn verify(counts: &[usize], probabilities: &[f64]) {
    assert_eq!(counts.iter().sum::<usize>(), 200_000);
    for (&count, &p) in counts.iter().zip(probabilities) {
        assert!(count > 0);
        let sigma = (count as f64 - 200_000.0 * p).abs() / (200_000.0 * p * (1.0 - p)).sqrt();
        assert!(
            sigma <= 5.0,
            "count {count}, probability {p}, sigma {sigma}"
        );
    }
}
#[test]
fn unary_samples_seed_every_row_uniformly() {
    let dataset = NeighborhoodDataset::new(0..10);
    let mut root = SampleRandom::new(0);
    let mut counts = [0; 10];
    for _ in 0..200_000 {
        let seed = root.index(NonZeroUsize::new(usize::MAX).unwrap()) as u64;
        let sample = dataset
            .samples(SizeSampler::exactly(1).unwrap(), seed)
            .next()
            .unwrap()
            .unwrap();
        counts[sample.members()[0]] += 1;
    }
    verify(&counts, &[0.1; 10]);
}
#[test]
fn binary_samples_weight_members_at_each_requested_position() {
    let probes = NeighborhoodDataset::new(["probe"]);
    let rows = NeighborhoodDataset::new(
        [2, 3, 5]
            .into_iter()
            .enumerate()
            .flat_map(|(bucket, n)| (0..n).map(move |i| ("probe", bucket, i))),
    );
    let join = probes.join(&rows, |key, row| *key == row.0);
    for at in [1, 2, 5] {
        let mut root = SampleRandom::new(0);
        let mut counts = [0; 3];
        for _ in 0..200_000 {
            let seed = root.index(NonZeroUsize::new(usize::MAX).unwrap()) as u64;
            let sample = join
                .samples_for(
                    "probe",
                    PillarSampler(SizeSampler::exactly(at).unwrap()),
                    seed,
                )
                .next()
                .unwrap()
                .unwrap();
            counts[sample.members()[at - 1].1] += 1;
        }
        verify(&counts, &[0.2, 0.3, 0.5]);
    }
}
