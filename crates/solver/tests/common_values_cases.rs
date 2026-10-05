// Adapted from Community SampleValueRangesTest and SampleValueRangesBiasIT.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{cell::Cell, collections::BTreeMap, sync::Arc};
use timetuck::*;
fn range<V: Clone + Eq + 'static>(v: impl IntoIterator<Item = V>) -> SharedValueRange<'static, V> {
    Arc::new(EnumeratedValueRange::new(v))
}
fn common<V: Clone + Eq + 'static>(
    r: impl IntoIterator<Item = SharedValueRange<'static, V>>,
) -> CommonValueRanges<'static, V> {
    CommonValueRanges::new(r).unwrap()
}
#[test]
fn of_sample_based_excludes_null_and_checks_legality_across_every_member() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec!["v1", "v2"]).allowing_unassigned(),
        Variable::new(vec!["v1", "v2"]).allowing_unassigned(),
        Variable::new(vec!["v1", "v3"]).allowing_unassigned(),
    ])
    .unwrap();
    let ranges = CommonValueRanges::for_variables(&p, &[0, 1, 2]).unwrap();
    assert!(ranges.contains(Some(&"v1")));
    assert!(!ranges.contains(Some(&"v2")));
    assert!(!ranges.contains(None));
}
#[test]
fn of_solution_scoped_reads_one_range_for_the_whole_sample() {
    let shared = range(["v1", "v2"]);
    let reads = Cell::new(0);
    let ranges = CommonValueRanges::from_shared_with(|| {
        reads.set(reads.get() + 1);
        shared.clone()
    });
    assert_eq!(ranges.range_count(), 1);
    assert!(ranges.contains(Some(&"v1")));
    assert!(ranges.contains(Some(&"v2")));
    assert!(!ranges.contains(None));
    assert_eq!(reads.get(), 1);
}
#[test]
fn bail_out_size_of_clamps_huge_size_instead_of_overflowing() {
    assert!(intersection_probe_budget(i64::MAX as u128) > 0);
}
#[test]
fn pick_exactly_returns_null_when_intersection_is_empty() {
    assert_eq!(
        common([range(["a", "b"]), range(["c", "d"])])
            .pick_exact(None, 0, 100)
            .unwrap(),
        None
    );
}
#[test]
fn pick_exactly_excludes_the_given_value() {
    assert_eq!(
        common([range(["only"])])
            .pick_exact(Some(&"only"), 0, 100)
            .unwrap(),
        None
    );
}
#[test]
fn find_destination_single_range_returns_a_member() {
    assert!(
        ["a", "b", "c"].contains(
            &common([range(["a", "b", "c"])])
                .find_target(None, 0, CommonValueBudget::default())
                .unwrap()
                .unwrap()
        )
    );
}
#[test]
fn find_destination_single_range_excludes_the_given_value_even_as_the_only_candidate() {
    assert_eq!(
        common([range(["only"])])
            .find_target(Some(&"only"), 0, CommonValueBudget::default())
            .unwrap(),
        None
    );
}
#[test]
fn find_destination_multi_range_returns_an_intersection_member_or_null_when_disjoint() {
    assert_eq!(
        common([range(["a", "b"]), range(["b", "c"])])
            .find_target(None, 0, CommonValueBudget::default())
            .unwrap(),
        Some("b")
    );
    assert_eq!(
        common([range(["a"]), range(["b"])])
            .find_target(None, 0, CommonValueBudget::default())
            .unwrap(),
        None
    );
}
#[test]
fn equals_ignores_order_but_not_membership() {
    let (a, b, c) = (range(["a"]), range(["b"]), range(["c"]));
    assert_eq!(
        common([a.clone(), b.clone()]),
        common([b.clone(), a.clone()])
    );
    assert_ne!(common([a.clone(), b.clone()]), common([a.clone(), c]));
    assert_ne!(common([a.clone()]), common([a, b]));
}
fn assert_uniform(mut pick: impl FnMut(u64) -> Option<i32>, expected: &[i32]) {
    const TRIALS: usize = 200_000;
    // Root-seeded native random draws replace Java Random.splitFrom; no trace parity.
    let seeds = IntegerValueRange::new(i64::MIN, i64::MAX, 1).unwrap();
    let mut count = BTreeMap::<i32, usize>::new();
    for seed in seeds.random_iter(0).take(TRIALS) {
        let value = pick(seed as u64).unwrap();
        assert!(expected.contains(&value));
        *count.entry(value).or_default() += 1;
    }
    let p = 1.0 / expected.len() as f64;
    let mean = TRIALS as f64 * p;
    let sigma = (TRIALS as f64 * p * (1.0 - p)).sqrt();
    for v in expected {
        let observed = *count.get(v).expect("every category must be reached");
        let deviation = (observed as f64 - mean).abs() / sigma;
        assert!(
            deviation <= 5.0,
            "value={v}, count={observed}, expected={mean}, sigma={deviation}"
        );
    }
}
#[test]
fn pick_exactly_is_uniform_over_the_intersection() {
    let ranges = common([range([1, 2, 3, 4, 5]), range([2, 3, 4, 6, 7])]);
    for (excluded, expected) in [(None, vec![2, 3, 4]), (Some(3), vec![2, 4])] {
        assert_uniform(
            |seed| ranges.pick_exact(excluded.as_ref(), seed, 100).unwrap(),
            &expected,
        );
    }
}
#[test]
fn find_destination_is_uniform_over_the_intersection_via_sampling() {
    let ranges = common([range(1..=10), range([1, 2, 3, 4, 5, 6, 7, 8, 100, 200])]);
    for (excluded, expected) in [
        (None, vec![1, 2, 3, 4, 5, 6, 7, 8]),
        (Some(5), vec![1, 2, 3, 4, 6, 7, 8]),
    ] {
        assert_uniform(
            |seed| {
                ranges
                    .find_target(
                        excluded.as_ref(),
                        seed,
                        CommonValueBudget {
                            random_draws: 100,
                            max_exact_values: 100,
                        },
                    )
                    .unwrap()
            },
            &expected,
        );
    }
}
