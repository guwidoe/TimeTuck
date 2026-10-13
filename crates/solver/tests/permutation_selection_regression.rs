// Original native controls; no upstream method credit.
use std::{collections::BTreeSet, num::NonZeroU128};
use timetuck::*;
#[test]
fn every_bounded_draw_sequence_bijects_to_a_live_handle_permutation() {
    for size in 0..=6 {
        let mut list = EntryList::new();
        let handles: Vec<_> = (0..size).map(|i| list.push(i)).collect();
        fn enumerate(
            list: &EntryList<usize>,
            handles: &[ListEntry<usize>],
            draws: &mut Vec<u128>,
            remaining: usize,
            seen: &mut BTreeSet<Vec<usize>>,
        ) {
            if remaining > 1 {
                for d in 0..remaining {
                    draws.push(d as u128);
                    enumerate(list, handles, draws, remaining - 1, seen);
                    draws.pop();
                }
                return;
            }
            let mut input = draws.iter().copied();
            let entries: Vec<_> = list
                .randomized_entries_with(|_| input.next().unwrap())
                .map(Result::unwrap)
                .collect();
            let values: Vec<_> = entries.iter().map(|e| *e.value()).collect();
            assert!(seen.insert(values.clone()));
            let mut sorted = values;
            sorted.sort();
            assert_eq!(sorted, (0..handles.len()).collect::<Vec<_>>());
            for e in entries {
                assert!(e.same_entry(&handles[*e.value()]));
            }
        }
        let mut seen = BTreeSet::new();
        enumerate(&list, &handles, &mut vec![], size, &mut seen);
        assert_eq!(seen.len(), (1..=size).product::<usize>());
    }
}
#[test]
fn removed_equal_payload_handles_and_invalid_draws_retain_real_membership() {
    struct NoClone;
    let mut list = EntryList::new();
    let handles: Vec<_> = (0..80).map(|_| list.push(NoClone)).collect();
    for h in handles.iter().step_by(3) {
        list.remove(h).unwrap();
    }
    let a: Vec<_> = list.randomized_entries(37).map(Result::unwrap).collect();
    let b: Vec<_> = list.randomized_entries(37).map(Result::unwrap).collect();
    assert_eq!(a.len(), list.len());
    assert!(a.iter().zip(&b).all(|(a, b)| a.same_entry(b)));
    for (i, h) in handles.iter().enumerate() {
        assert_eq!(a.iter().any(|e| e.same_entry(h)), i % 3 != 0);
    }
    let mut it = list.randomized_entries_with(|n| n);
    assert!(it.next().unwrap().is_err());
    assert!(it.next().is_none());
    assert!(it.next().is_none());
    drop(it);
    list.clear();
    assert!(
        list.randomized_entries_with(|_| panic!("empty draw"))
            .next()
            .is_none()
    );
}
#[test]
fn wide_draws_preserve_large_bounds_seed_and_distribution_controls() {
    for bound in [
        1,
        2,
        17,
        i32::MAX as u128 + 10,
        u64::MAX as u128 + 1,
        (1_u128 << 100) + 17,
        u128::MAX,
    ] {
        let bound = NonZeroU128::new(bound).unwrap();
        let mut a = SampleRandom::new(37);
        let mut b = SampleRandom::new(37);
        for _ in 0..1000 {
            let x = a.wide_index(bound);
            assert!(x < bound.get());
            assert_eq!(x, b.wide_index(bound));
        }
    }
    let mut random = SampleRandom::new(37);
    let mut bins = [0; 8];
    for _ in 0..80_000 {
        bins[random.wide_index(NonZeroU128::new(8).unwrap()) as usize] += 1;
    }
    assert!(bins.into_iter().all(|n| (9_300..=10_700).contains(&n)));
    let mut random = SampleRandom::new(37);
    assert!(
        (0..100)
            .any(|_| random.wide_index(NonZeroU128::new(u128::MAX).unwrap()) > u64::MAX as u128)
    );
}
#[test]
fn pillar_weight_zero_and_candidate_ceiling_affect_actual_moves() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(1),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let config = PillarSwapSelection::default();
    let selected = config.select(&p, &a, 2).unwrap();
    assert_eq!(selected.len(), 2);
    assert!(matches!(
        config.select(&p, &a, 1),
        Err(PillarSwapSelectionError::Weight(
            WeightedSelectionError::CandidateLimit { maximum: 1 }
        ))
    ));
    let zero = config
        .with_probability_weight(0.0)
        .unwrap()
        .select(&p, &a, 2)
        .unwrap();
    assert_eq!(zero.len(), 2);
    assert_eq!(zero.active_len(), 0);
    assert!(zero.samples(37).next().is_none());
    assert!(
        PillarSwapSelection::default()
            .with_probability_weight(f64::NAN)
            .is_err()
    );
}
