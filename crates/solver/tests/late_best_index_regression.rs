//! Original cache-read, epoch, panic and independent history controls.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn unchanged_indices_skip_factories_and_checks_even_with_a_different_score() {
    let mut state = LateBestLevelState::at_best_step(HardMediumSoftScore::new(-1, -2, -3), 7);
    let reads = Cell::new(0);
    for _ in 0..100 {
        state.refresh_best_step(7, || {
            reads.set(reads.get() + 1);
            HardMediumSoftScore::ZERO
        });
        assert!(!state.higher_reset_levels(&7, || {
            reads.set(reads.get() + 1);
            &HardMediumSoftScore::ZERO
        }));
    }
    assert_eq!(reads.get(), 0);
    assert_eq!(
        state.baseline_score(),
        &HardMediumSoftScore::new(-1, -2, -3)
    );
    assert!(state.higher_reset_levels(&8, || {
        reads.set(reads.get() + 1);
        &HardMediumSoftScore::ZERO
    }));
    assert_eq!(reads.get(), 1);
    state.refresh_best_step(8, || {
        reads.set(reads.get() + 1);
        HardMediumSoftScore::ZERO
    });
    assert_eq!(reads.get(), 2);
    assert_eq!(state.baseline_index(), &8);
}
#[test]
fn wrapping_and_nonmonotonic_epochs_compare_against_the_latest_baseline() {
    let mut state = LateBestLevelState::at_best_step(HardSoftScore::new(-1, -10), u64::MAX);
    let better = HardSoftScore::new(0, -100);
    assert!(state.higher_reset_levels(&0, || &better));
    state.refresh_best_step(0, || better);
    assert!(!state.higher_reset_levels(&0, || panic!("same index read")));
    let soft = HardSoftScore::new(0, 100);
    assert!(!state.higher_reset_levels(&u64::MAX, || &soft));
    state.refresh_best_step(u64::MAX, || soft);
    assert_eq!(state.baseline_score(), &soft);
}
#[test]
fn failed_score_factories_preserve_both_cached_fields() {
    let mut state = LateBestLevelState::at_best_step(HardSoftScore::new(-1, -5), 0);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || state.refresh_best_step(1, || panic!("clone failed"))
        ))
        .is_err()
    );
    assert_eq!(state.baseline_index(), &0);
    assert_eq!(state.baseline_score(), &HardSoftScore::new(-1, -5));
    assert!(
        catch_unwind(AssertUnwindSafe(
            || state.higher_reset_levels(&1, || panic!("read failed"))
        ))
        .is_err()
    );
    assert_eq!(state.baseline_index(), &0);
    state.refresh_best_step(1, || HardSoftScore::ZERO);
    assert_eq!(state.baseline_score(), &HardSoftScore::ZERO);
}
fn independent_history<S: Score>(initial: S, scores: Vec<S>, reset: impl Fn(&S, &S) -> bool) {
    for size in [1, 2, 7, 31] {
        for hill in [false, true] {
            let mut acceptor = LateAcceptance::new(size, hill, initial.clone()).unwrap();
            let mut history = vec![initial.clone(); size];
            let mut slot = 0;
            let mut best = initial.clone();
            let mut current = initial.clone();
            for score in &scores {
                let expected = *score >= history[slot] || (hill && *score >= current);
                assert_eq!(acceptor.accepts(current.clone(), score.clone()), expected);
                let improves = *score > best;
                if improves && reset(score, &best) {
                    history.fill(score.clone());
                }
                history[slot] = score.clone();
                slot = (slot + 1) % size;
                if improves {
                    best = score.clone();
                }
                current = score.clone();
                acceptor.step_ended(score.clone());
                assert_eq!(acceptor.threshold(), history[slot]);
                assert_eq!(acceptor.history(), history.as_slice());
            }
        }
    }
}
#[test]
fn integrated_acceptor_matches_independent_full_histories_and_decisions() {
    let raw: Vec<_> = (0..4000)
        .map(|i| (i / 1000 - 2, (i * 17) % 211 - 100, (i * 31) % 317 - 200))
        .collect();
    independent_history(
        HardSoftScore::new(-3, -1000),
        raw.iter()
            .map(|&(h, _, s)| HardSoftScore::new(h, s))
            .collect(),
        |a, b| a.hard() > b.hard(),
    );
    independent_history(
        HardMediumSoftScore::new(-3, -1000, -1000),
        raw.iter()
            .map(|&(h, m, s)| HardMediumSoftScore::new(h, m, s))
            .collect(),
        |a, b| (a.hard(), a.medium()) > (b.hard(), b.medium()),
    );
    independent_history(
        BendableScore::new([-3, -1000], [-1000]),
        raw.iter()
            .map(|&(h, m, s)| BendableScore::new([h, m], [s]))
            .collect(),
        |a, b| a.hard_levels() > b.hard_levels(),
    );
}
