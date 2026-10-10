//! Original wide-rank arithmetic and model integration checks.
use timetuck::*;
fn triangle(n: usize) -> u128 {
    let n = n as u128;
    n * (n + 1) / 2
}
fn oracle(n: usize, min: usize, max: usize, rank: u128) -> Option<std::ops::Range<usize>> {
    let longest = n.min(max);
    let width = (n - longest + 1) as u128;
    let rows = longest - min + 1;
    let prefix = |k: usize| {
        let k = k as u128;
        k * (2 * width + k - 1) / 2
    };
    if rank >= prefix(rows) {
        return None;
    }
    let (mut low, mut high) = (0, rows);
    while low < high {
        let mid = low + (high - low) / 2;
        if prefix(mid + 1) <= rank {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    let start = (rank - prefix(low)) as usize;
    Some(start..start + (longest - low))
}
#[test]
fn every_small_rank_matches_explicit_spans_and_bounds() {
    for n in 1..35 {
        for min in 1..=n {
            for max in min..=n + 2 {
                let index = SubListRankIndex::new(n, min, max).unwrap();
                let expected: Vec<_> = (min..=max.min(n))
                    .rev()
                    .flat_map(|len| (0..=n - len).map(move |start| start..start + len))
                    .collect();
                assert_eq!(index.span_count(), expected.len() as u128);
                for (rank, span) in expected.into_iter().enumerate() {
                    assert_eq!(index.at(rank as u128), Some(span));
                }
                assert_eq!(index.at(index.span_count()), None);
                assert_eq!(index.at(u128::MAX), None);
            }
        }
    }
}
#[test]
fn maximum_virtual_lists_and_triangular_boundaries_match_independent_binary_oracle() {
    for n in [
        1,
        2,
        3,
        46340,
        65535,
        1usize << 31,
        usize::MAX / 2,
        usize::MAX - 1,
        usize::MAX,
    ] {
        for (min, max) in [
            (1, n),
            (1, n.min(5)),
            (n, n),
            (n / 2 + 1, n),
            (n.saturating_sub(3).max(1), n),
        ] {
            let index = SubListRankIndex::new(n, min, max).unwrap();
            let count = index.span_count();
            let mut ranks = vec![0, count - 1, count / 2];
            let base = triangle(n - max);
            for row in [
                n - max,
                (n - max).saturating_add(1).min(n - min),
                n - min,
                n / 2,
            ] {
                if let Some(boundary) = triangle(row).checked_sub(base) {
                    ranks.extend([
                        boundary.saturating_sub(1),
                        boundary,
                        boundary.saturating_add(1),
                    ]);
                }
            }
            let mut random = 7u128;
            for _ in 0..128 {
                random = random
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ranks.push(random % count);
            }
            for rank in ranks {
                assert_eq!(
                    index.at(rank),
                    oracle(n, min, max, rank),
                    "n={n} min={min} max={max} rank={rank}"
                );
            }
        }
    }
}
#[test]
fn multi_entity_global_and_local_ranks_preserve_pinned_prefixes_and_draw_order() {
    let p = ListProblem::new(
        (0..10).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3]).with_pinned_prefix(1),
            ListEntity::new(vec![4, 5]).with_pinned(true),
            ListEntity::new(vec![6, 7, 8, 9]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let bounds = SubListSize::new(1, 3).unwrap();
    let index = SubListSelection::new(&p, &a, bounds).unwrap();
    let mut expected = vec![];
    for (entity, prefix, available) in [(0, 1, 3), (2, 0, 4)] {
        let mut rank = 0;
        for len in (1..=3).rev() {
            for start in 0..=available - len {
                let span = ListRange {
                    entity,
                    start: start + prefix,
                    end: start + prefix + len,
                };
                assert_eq!(index.at_entity(entity, rank), Some(span));
                expected.push(span);
                rank += 1;
            }
        }
        assert_eq!(index.at_entity(entity, rank), None);
    }
    assert_eq!(index.entity_len(1), 0);
    assert_eq!(index.at_entity(1, 0), None);
    assert_eq!(index.len(), expected.len() as u128);
    let mut rank = 0;
    let actual: Vec<_> = index
        .random_iter_with(|bound| {
            assert_eq!(bound, expected.len() as u128);
            let r = rank;
            rank += 1;
            r
        })
        .take(expected.len())
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(index.at(index.len()), None);
}
#[test]
fn invalid_draws_fail_without_allocating_or_retrying() {
    let index = SubListRankIndex::new(usize::MAX, 1, usize::MAX).unwrap();
    let calls = std::cell::Cell::new(0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        index.draw_with(|bound| {
            calls.set(calls.get() + 1);
            bound
        })
    }));
    assert!(result.is_err());
    assert_eq!(calls.get(), 1);
}
