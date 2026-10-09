// Adapted from pinned Community SubListSamplingBiasIT and BiasReport.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{collections::BTreeMap, num::NonZeroUsize};
use timetuck::*;
const TRIALS: usize = 200_000;
fn verify(counts: impl IntoIterator<Item = (usize, f64)>) {
    let mut total = 0;
    for (count, probability) in counts {
        total += count;
        assert!(count > 0);
        let sigma = (count as f64 - TRIALS as f64 * probability).abs()
            / (TRIALS as f64 * probability * (1.0 - probability)).sqrt();
        assert!(
            sigma <= 5.0,
            "count={count}, p={probability}, sigma={sigma}"
        );
    }
    assert_eq!(total, TRIALS);
}
#[test]
fn spans_are_uniform_over_every_admissible_pair() {
    let p = ListProblem::new((0..7).collect(), vec![ListEntity::new((0..7).collect())])
        .unwrap()
        .allowing_unassigned();
    let a = p.initial_assignment();
    let index = ValueSeededSubListSelection::new(&p, &a, SubListSize::new(2, 5).unwrap()).unwrap();
    let mut counts: BTreeMap<_, usize> = (2..=5)
        .flat_map(|length| (0..=7 - length).map(move |start| ((start, start + length), 0)))
        .collect();
    assert_eq!(counts.len(), 18);
    let mut spans = index.random_iter(std::iter::repeat(&a.lists()[0][0]), TRIALS, 0);
    for _ in 0..TRIALS {
        let span = spans.next().unwrap();
        assert_eq!(span.entity, 0);
        *counts
            .get_mut(&(span.start, span.end))
            .expect("admissible span") += 1;
    }
    assert!(spans.next().is_none());
    verify(counts.into_values().map(|count| (count, 1.0 / 18.0)));
}
#[test]
fn seeded_entities_are_weighted_by_unpinned_size() {
    let p = ListProblem::new(
        (0..10).collect(),
        vec![
            ListEntity::new((0..2).collect()),
            ListEntity::new((2..5).collect()),
            ListEntity::new((5..10).collect()),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let index =
        ValueSeededSubListSelection::new(&p, &a, SubListSize::new(1, i32::MAX as usize).unwrap())
            .unwrap();
    let mut picker = SampleRandom::new(1);
    let seeds =
        std::iter::repeat_with(|| &p.values()[picker.index(NonZeroUsize::new(10).unwrap())]);
    let mut spans = index.random_iter(seeds, TRIALS, 0);
    let mut counts = [0; 3];
    for _ in 0..TRIALS {
        counts[spans.next().unwrap().entity] += 1;
    }
    assert!(spans.next().is_none());
    verify(counts.into_iter().zip([0.2, 0.3, 0.5]));
}
