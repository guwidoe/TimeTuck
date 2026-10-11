// Original native cache regressions; no upstream method credit.
use std::num::NonZeroUsize;
use timetuck::*;

// Rebuild the complete geometrically eligible candidate row on EVERY draw using
// the uncached ordered API, then validate the selected candidate in the actual
// dependency-constrained model. Thus dependency rejection consumes its draw.
fn reference(
    plain: &ListProblem<i32>,
    actual: &ListProblem<i32>,
    assignment: &ListAssignment<i32>,
    provider: &SampledListValueSwapProvider<i32>,
    step: u64,
) -> Vec<ListMove<i32>> {
    let selectable = |value: &&i32| {
        for (owner, row) in assignment.lists().iter().enumerate() {
            if let Some(index) = row.iter().position(|v| v == *value) {
                let entity = &plain.entities()[owner];
                return !entity.is_pinned() && index >= entity.pinned_prefix();
            }
        }
        plain.allows_unassigned()
    };
    let left: Vec<_> = provider.left.iter().filter(selectable).copied().collect();
    let right: Vec<_> = provider.right.iter().filter(selectable).copied().collect();
    let mut rng = SampleRandom::new(provider.seed.wrapping_add(step));
    let draw = |rng: &mut SampleRandom, n| {
        if n == 1 {
            0
        } else {
            rng.index(NonZeroUsize::new(n).unwrap())
        }
    };
    let mut result = vec![];
    if left.is_empty() || right.is_empty() {
        return result;
    }
    for _ in 0..provider.attempts_per_step {
        let l = left[draw(&mut rng, left.len())];
        let candidates: Vec<_> = list_value_swap_moves(
            plain,
            assignment,
            &[l],
            &right,
            ListSwapPairing::Cartesian,
            provider.include_noops,
        )
        .unwrap()
        .collect();
        if candidates.is_empty() {
            continue;
        }
        let movement = candidates[draw(&mut rng, candidates.len())].clone();
        let mut scratch = assignment.clone();
        let valid = actual.apply(&mut scratch, &movement).is_ok();
        if valid {
            result.push(movement);
        }
    }
    result
}
fn compare(
    p: &ListProblem<i32>,
    actual: &ListProblem<i32>,
    a: &ListAssignment<i32>,
    c: &SampledListValueSwapProvider<i32>,
) {
    for step in [0, 1, 17] {
        let expected = reference(p, actual, a, c, step);
        assert_eq!(
            c.select(actual, a, step).unwrap().collect::<Vec<_>>(),
            expected
        );
        let mut partial = c.select(actual, a, step).unwrap();
        assert_eq!(partial.next(), expected.first().cloned());
        assert_eq!(
            c.select(actual, a, step).unwrap().collect::<Vec<_>>(),
            expected
        );
    }
}
#[test]
fn cached_rows_preserve_duplicates_ranges_pins_missing_values_and_reopening() {
    let p = ListProblem::from_entity_ranges(vec![
        ListEntity::new(vec![0, 1])
            .with_pinned_prefix(1)
            .with_value_range(vec![0, 1, 2, 4]),
        ListEntity::new(vec![2]).with_value_range(vec![1, 2]),
        ListEntity::new(vec![3])
            .with_pinned(true)
            .with_value_range(vec![3]),
        ListEntity::new(vec![5]).with_value_range(vec![5, 6]),
    ])
    .unwrap()
    .allowing_unassigned();
    let mut a = p.initial_assignment();
    for include_noops in [false, true] {
        for left in [vec![1], vec![0, 1, 1, 3, 5, 6], vec![4, 6]] {
            let c = SampledListValueSwapProvider {
                left,
                right: [0, 2, 2, 3, 4].repeat(32),
                include_noops,
                seed: u64::MAX,
                attempts_per_step: 200,
            };
            compare(&p, &p, &a, &c);
            let first = c
                .select(&p, &a, 0)
                .unwrap()
                .find(|m| *m != ListMove::Composite(vec![]));
            if let Some(movement) = first {
                p.apply(&mut a, &movement).unwrap().commit();
                compare(&p, &p, &a, &c);
            }
        }
    }
}
#[test]
fn cache_thresholds_and_churning_left_keys_match_uncached_occurrence_sampling() {
    let values: Vec<_> = (0..20).collect();
    let p = ListProblem::new(values.clone(), vec![ListEntity::new(values)]).unwrap();
    let a = p.initial_assignment();
    for right_len in [1, 63, 64, 128] {
        for attempts_per_step in [3, 4, 300] {
            for left in [vec![0], vec![0, 0, 1], (0..20).collect()] {
                let c = SampledListValueSwapProvider {
                    left,
                    right: (0..right_len).map(|i| i % 20).collect(),
                    include_noops: false,
                    seed: 19,
                    attempts_per_step,
                };
                compare(&p, &p, &a, &c);
            }
        }
    }
}
#[test]
fn dependency_rejection_remains_after_right_draw_on_cache_hits() {
    let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0, 1, 2])]).unwrap();
    let actual = p
        .clone()
        .with_dependencies(vec![vec![], vec![0], vec![1]])
        .unwrap();
    let a = p.initial_assignment();
    let c = SampledListValueSwapProvider {
        left: vec![0, 1],
        right: [0, 1, 2, 2].repeat(32),
        include_noops: true,
        seed: 31,
        attempts_per_step: 300,
    };
    compare(&p, &actual, &a, &c);
    let results: Vec<_> = c.select(&actual, &a, 0).unwrap().collect();
    assert!(!results.is_empty() && results.len() < 300);
    assert!(results.iter().all(|m| *m == ListMove::Composite(vec![])));
}
