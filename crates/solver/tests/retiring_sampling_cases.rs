// Adapted from pinned Community retiring/filtering iterator tests, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{cell::Cell, collections::BTreeSet, num::NonZeroUsize};
use timetuck::*;
fn budget(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}

#[test]
fn empty_set() {
    let mut pool = RetiringRandom::<String>::new([], 0);
    assert!(pool.is_empty());
    assert_eq!(pool.next(), None);
    assert_eq!(pool.retire(), Err(NoDrawToRetire));
}
#[test]
fn single_element_set_pick_and_retire() {
    let mut pool = RetiringRandom::new(["A"], 0);
    assert!(!pool.is_empty());
    assert_eq!(pool.next(), Some("A"));
    pool.retire().unwrap();
    assert!(pool.is_empty());
    assert_eq!(pool.next(), None);
}
#[test]
fn multiple_element_set() {
    let values = ["A", "B", "C", "D", "E"];
    let mut pool = RetiringRandom::new(values, 0);
    assert!(!pool.is_empty());
    assert!(values.contains(&pool.next().unwrap()));
}
#[test]
fn pick_does_not_modify_set() {
    let values = ["A", "B", "C"];
    let mut pool = RetiringRandom::new(values, 0);
    let draws: Vec<_> = pool.by_ref().take(3).collect();
    assert!(!pool.is_empty());
    assert!(draws.iter().all(|v| values.contains(v)));
    assert_eq!(pool.remaining(), 3);
}
#[test]
fn retire_all_elements() {
    let values = ["A", "B", "C", "D", "E"];
    let mut pool = RetiringRandom::new(values, 0);
    let mut seen = BTreeSet::new();
    for _ in 0..5 {
        assert!(!pool.is_empty());
        seen.insert(pool.next().unwrap());
        pool.retire().unwrap();
    }
    assert!(pool.is_empty());
    assert_eq!(seen, BTreeSet::from(values));
    assert_eq!(pool.next(), None);
}
#[test]
fn retire_all_elements_across_upgrade_threshold() {
    let mut pool = RetiringRandom::new(0..100, 0);
    let mut seen = BTreeSet::new();
    for _ in 0..100 {
        assert!(!pool.is_empty());
        seen.insert(pool.next().unwrap());
        pool.retire().unwrap();
    }
    assert!(pool.is_empty());
    assert_eq!(seen, (0..100).collect());
}
#[test]
fn drain_skips_gaps_and_returns_every_live_element_once() {
    let mut pool =
        RetiringRandom::from_slots((0..100).map(|i| (i % 5 != 0).then_some(i)).collect(), 0);
    assert!(pool.slot_count() > pool.remaining());
    let mut actual = Vec::new();
    while let Some(value) = pool.next() {
        actual.push(value);
        pool.retire().unwrap();
    }
    actual.sort();
    assert_eq!(actual, (0..100).filter(|i| i % 5 != 0).collect::<Vec<_>>());
}

#[test]
fn no_bail_out_behaves_like_a_filtered_finite_iterator() {
    let mut filter = BoundedFilter::new(1..=5, budget(usize::MAX), |v: &i32| v % 2 == 0);
    assert_eq!(filter.probe(), FilterProbe::Found(2));
    assert_eq!(filter.probe(), FilterProbe::Found(4));
    assert_eq!(filter.probe(), FilterProbe::SourceExhausted);
    assert_eq!(filter.probe(), FilterProbe::SourceExhausted);
}
#[test]
fn bail_out_rejecting_filter_on_never_ending_delegate_still_terminates() {
    let mut filter = BoundedFilter::new(std::iter::repeat(0), budget(1000), |_: &i32| false);
    assert_eq!(filter.probe(), FilterProbe::BudgetExhausted);
}
#[test]
fn bail_out_accepting_filter_still_finds_its_match() {
    let mut random = SampleRandom::new(0);
    let source = std::iter::from_fn(|| Some(random.index(budget(1000))));
    let mut filter = BoundedFilter::new(source, budget(10_000), |v: &usize| *v == 0 || *v == 1);
    assert!(matches!(filter.probe(), FilterProbe::Found(0 | 1)));
}
#[test]
fn bail_out_resets_on_every_probe_call() {
    let mut filter = BoundedFilter::new(0.., budget(1000), |v: &usize| *v >= 1000);
    assert_eq!(filter.probe(), FilterProbe::BudgetExhausted);
    assert_eq!(filter.probe(), FilterProbe::Found(1000));
}

#[test]
fn a_left_is_retried_up_to_probe_attempt_count_before_giving_up() {
    let attempts = budget(8); // Explicit native configuration replaces the Java constant.
    let calls = Cell::new(0);
    let mut pairs = RetiringPairs::new(["left"], attempts, 0, |_: &&str, _: &mut SampleRandom| {
        calls.set(calls.get() + 1);
        (calls.get() == attempts.get()).then_some("right")
    });
    assert_eq!(pairs.next(), Some(("left", "right")));
    assert_eq!(
        pairs.remaining_lefts(),
        1,
        "successful left was not retired"
    );
    assert_eq!(calls.get(), attempts.get());
}
#[test]
fn a_left_with_no_match_is_retired_after_exactly_probe_attempt_count_probes() {
    let calls = Cell::new(0);
    let attempts = budget(8);
    let mut pairs = RetiringPairs::new(0..4, attempts, 0, |_: &usize, _: &mut SampleRandom| {
        calls.set(calls.get() + 1);
        None::<()>
    });
    assert_eq!(pairs.next(), None);
    assert_eq!(calls.get(), attempts.get() * 4);
}

fn assert_terminates(empty_left: bool, empty_right: bool) {
    let problem = FiniteProblem::new(
        (0..3)
            .map(|v| Variable::new(vec![0, 1, 2]).with_initial(v))
            .collect(),
    )
    .unwrap();
    let left = (0..problem.variables().len()).filter(|_| !empty_left);
    let right = NeighborhoodDataset::new((0..3).filter(|_| !empty_right));
    let mut moves = RetiringPairs::new(
        left,
        budget(8),
        0,
        |_: &usize, random: &mut SampleRandom| {
            BoundedFilter::new(right.exhaustive_iter(random), budget(3), |_: &i32| false)
                .probe()
                .into_match()
        },
    )
    .map(|(variable, value)| Move::Change {
        variable,
        value: Some(value),
    });
    assert_eq!(moves.next(), None);
}
#[test]
fn empty_right_dataset_terminates() {
    assert_terminates(false, true);
}
#[test]
fn empty_left_dataset_terminates() {
    assert_terminates(true, false);
}
#[test]
fn filtering_joiner_rejects_every_pair_terminates() {
    assert_terminates(false, false);
}
