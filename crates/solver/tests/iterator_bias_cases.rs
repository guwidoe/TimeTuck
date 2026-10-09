// Adapted from pinned Community IteratorBiasIT and its test-only helpers.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{collections::BTreeMap, num::NonZeroUsize};
use timetuck::*;
fn seed(root: &mut SampleRandom) -> u64 {
    root.index(NonZeroUsize::new(usize::MAX).unwrap()) as u64
}
fn uniform(counts: &[usize], trials: usize) {
    weights(
        counts,
        trials,
        &vec![1.0 / counts.len() as f64; counts.len()],
    );
}
fn weights(counts: &[usize], trials: usize, probabilities: &[f64]) {
    assert_eq!(counts.iter().sum::<usize>(), trials);
    for (&observed, &p) in counts.iter().zip(probabilities) {
        assert!(observed > 0);
        let sigma =
            (observed as f64 - trials as f64 * p).abs() / (trials as f64 * p * (1.0 - p)).sqrt();
        assert!(
            sigma <= 5.0,
            "observed={observed}, trials={trials}, p={p}, sigma={sigma}"
        );
    }
}
fn slots(n: usize, gapped: bool) -> Vec<Option<usize>> {
    if !gapped || n < 4 {
        return (0..n).map(Some).collect();
    }
    let budget = n / 4;
    let body = budget.saturating_sub(2);
    let spacing = if body == 0 {
        usize::MAX
    } else {
        n / (body + 1)
    };
    let mut result = vec![None];
    let mut gaps = 1;
    for i in 0..n {
        result.push(Some(i));
        if gaps < budget - 1 && i > 0 && i % spacing == 0 {
            result.push(None);
            gaps += 1;
        }
    }
    if gaps < budget {
        result.push(None);
    }
    assert!(result.len() > n);
    result
}
#[test]
fn repeating_uniform_at_every_requested_draw_in_both_layouts() {
    let checkpoints = [1, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
    let trials = 1_000_000;
    for gapped in [false, true] {
        let layout = slots(100, gapped);
        let mut counts = vec![vec![0; 100]; checkpoints.len()];
        let mut root = SampleRandom::new(0);
        for _ in 0..trials {
            let mut draw = RetiringRandom::from_slots(layout.clone(), seed(&mut root));
            let mut next = 0;
            for at in 1..=100 {
                let value = *draw.draw().unwrap();
                if at == checkpoints[next] {
                    counts[next][value] += 1;
                    next += 1;
                }
            }
        }
        for count in counts {
            uniform(&count, trials);
        }
    }
}
#[test]
fn retiring_uniform_after_interior_removals_in_both_layouts() {
    let trials = 1_000_000;
    for gapped in [false, true] {
        let layout = slots(20, gapped);
        let mut root = SampleRandom::new(0);
        let mut counts = vec![0; 20];
        for _ in 0..trials {
            let mut draw = RetiringRandom::from_slots(layout.clone(), seed(&mut root));
            for target in [5, 12] {
                let mut found = false;
                for _ in 0..100_000 {
                    if draw.draw() == Some(&target) {
                        draw.retire().unwrap();
                        found = true;
                        break;
                    }
                }
                assert!(found, "retirement draw limit exceeded");
            }
            counts[*draw.draw().unwrap()] += 1;
        }
        assert_eq!(counts[5], 0);
        assert_eq!(counts[12], 0);
        let counts: Vec<_> = counts
            .into_iter()
            .enumerate()
            .filter_map(|(i, n)| (![5, 12].contains(&i)).then_some(n))
            .collect();
        uniform(&counts, trials);
    }
}
#[test]
fn unique_drain_has_uniform_permutation_order_in_both_layouts() {
    let trials = 1_000_000;
    for gapped in [false, true] {
        let layout = slots(5, gapped);
        let mut root = SampleRandom::new(0);
        let mut counts = BTreeMap::new();
        for _ in 0..trials {
            let mut draw = RetiringRandom::from_slots(layout.clone(), seed(&mut root));
            let mut order = [0; 5];
            for value in &mut order {
                *value = *draw.draw().unwrap();
                draw.retire().unwrap();
            }
            assert!(draw.draw().is_none());
            let mut sorted = order;
            sorted.sort();
            assert_eq!(sorted, [0, 1, 2, 3, 4]);
            *counts.entry(order).or_insert(0) += 1;
        }
        assert_eq!(counts.len(), 120);
        uniform(&counts.into_values().collect::<Vec<_>>(), trials);
    }
}
fn index() -> CandidateIndex<i32, usize, usize> {
    let mut index = CandidateIndex::new();
    let mut id = 0;
    for (bucket, (key, n)) in [(10, 1), (20, 3), (30, 6)].into_iter().enumerate() {
        for _ in 0..n {
            index.insert(id, key, bucket).unwrap();
            id += 1;
        }
    }
    index
}
#[test]
fn comparison_repeating_weights_all_matching_buckets_in_both_directions() {
    let index = index();
    let trials = 200_000;
    for upper in [true, false] {
        let view = if upper {
            index.range(..=50)
        } else {
            index.range(10..)
        };
        let counts = &mut [0; 3];
        // One native stream replaces repeated wrappers sharing Java Random(0).
        for candidate in repeating_candidates(&view, 0).take(trials) {
            counts[candidate.unwrap().value] += 1;
        }
        weights(counts, trials, &[0.1, 0.3, 0.6]);
    }
}
#[test]
fn overlapping_memberships_do_not_overweight_shared_identity() {
    let mut index = MembershipCandidateIndex::new();
    index.put(0, vec!["X"], ());
    index.put(1, vec!["X", "Y"], ());
    index.put(2, vec!["Y"], ());
    let matches = index.query_any(["X", "Y"]);
    let trials = 200_000;
    let mut root = SampleRandom::new(0);
    let mut counts = [0; 3];
    for _ in 0..trials {
        counts[matches.unique(seed(&mut root)).next().unwrap().id] += 1;
    }
    uniform(&counts, trials);
}
#[test]
fn multibucket_unique_weights_every_requested_draw_for_both_query_kinds() {
    let index = index();
    let trials = 200_000;
    for comparison in [true, false] {
        let view = if comparison {
            index.range(..=50)
        } else {
            index.query_many([10, 20, 30])
        };
        for at in [1, 2, 10] {
            let mut root = SampleRandom::new(0);
            let mut counts = [0; 3];
            for _ in 0..trials {
                counts[view
                    .samples(seed(&mut root))
                    .nth(at - 1)
                    .unwrap()
                    .unwrap()
                    .value] += 1;
            }
            weights(&counts, trials, &[0.1, 0.3, 0.6]);
        }
    }
}
