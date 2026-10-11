//! Original reference-model, checkpoint and move-only payload regressions.
use std::collections::VecDeque;
use timetuck::*;
#[test]
fn mixed_wrap_growth_and_eviction_match_an_independent_deque() {
    for capacity in [1, 2, 3, 4, 5, 17] {
        let mut ring = TimedScoreRing::new(capacity).unwrap();
        let mut reference: VecDeque<(u128, i32)> = VecDeque::new();
        let mut random = 31_u64;
        let mut clock = 0_u128;
        for operation in 0..20_000 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            match (random >> 32) % 7 {
                0 => {
                    ring.clear();
                    reference.clear();
                }
                1 if !reference.is_empty() => {
                    ring.replace_latest_score(operation).unwrap();
                    reference.back_mut().unwrap().1 = operation;
                }
                2 | 3 if !reference.is_empty() => {
                    let cutoff = clock.saturating_sub(u128::from((random >> 8) % 20));
                    while reference.len() > 1 && reference[1].0 <= cutoff {
                        reference.pop_front();
                    }
                    assert_eq!(
                        ring.retain_floor(cutoff).unwrap(),
                        (reference[0].0, &reference[0].1)
                    );
                }
                _ => {
                    clock += u128::from((random >> 16) % 4);
                    ring.push(clock, operation).unwrap();
                    reference.push_back((clock, operation));
                }
            }
            assert_eq!(ring.len(), reference.len());
            assert_eq!(ring.front(), reference.front().map(|(t, s)| (*t, s)));
            assert_eq!(ring.back(), reference.back().map(|(t, s)| (*t, s)));
            let state = ring.state();
            assert_eq!(state.write, (state.read + state.len) % state.times.len());
            for (offset, (time, score)) in reference.iter().enumerate() {
                let i = (state.read + offset) % state.times.len();
                assert_eq!((state.times[i], state.scores[i]), (*time, Some(*score)));
            }
            // Validate restoration of wrapped, empty and full states, including
            // imported physical slots. Subsequent operations continue on it.
            if operation % 29 == 0 {
                ring = TimedScoreRing::from_state(state).unwrap();
            }
        }
    }
}
#[test]
fn growth_and_queries_preserve_move_only_scores_and_release_evicted_values() {
    use std::{cell::Cell, rc::Rc};
    #[derive(Debug)]
    struct Payload(usize, Rc<Cell<usize>>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.1.set(self.1.get() + 1);
        }
    }
    let dropped = Rc::new(Cell::new(0));
    let mut ring = TimedScoreRing::new(1).unwrap();
    for n in 0..300 {
        ring.push(n as u128, Payload(n, dropped.clone())).unwrap();
    }
    assert_eq!(dropped.get(), 0);
    assert_eq!(ring.retain_floor(127).unwrap().1.0, 127);
    assert_eq!(dropped.get(), 127);
    for n in 300..800 {
        ring.push(n as u128, Payload(n, dropped.clone())).unwrap();
    }
    assert_eq!(ring.retain_floor(u128::MAX).unwrap().1.0, 799);
    assert_eq!(dropped.get(), 799);
    assert_eq!(ring.len(), 1);
    ring.clear();
    assert_eq!(dropped.get(), 800);
    assert_eq!(ring.peek().unwrap_err(), TimedScoreRingError::Empty);
}
#[test]
fn zero_duplicate_and_maximum_times_and_backwards_rejection_are_explicit() {
    let mut ring = TimedScoreRing::new(1).unwrap();
    ring.push(0, 1).unwrap();
    ring.push(0, 2).unwrap();
    assert_eq!(ring.retain_floor(0).unwrap(), (0, &2));
    ring.push(u128::MAX, 3).unwrap();
    let before = ring.state();
    assert_eq!(
        ring.push(u128::MAX - 1, 4),
        Err(TimedScoreRingError::BackwardsTime)
    );
    assert_eq!(ring.state(), before);
    assert_eq!(ring.retain_floor(u128::MAX).unwrap(), (u128::MAX, &3));
    ring.clear();
    assert_eq!(
        ring.replace_latest_score(0),
        Err(TimedScoreRingError::Empty)
    );
    assert_eq!(
        TimedScoreRing::<i32>::new(0).unwrap_err(),
        TimedScoreRingError::Capacity
    );
    assert_eq!(
        TimedScoreRing::<i32>::new(usize::MAX).unwrap_err(),
        TimedScoreRingError::Capacity
    );
}
#[test]
fn checkpoints_reject_bad_cursors_lengths_missing_scores_and_time_order() {
    let base = TimedScoreRingState {
        read: 1,
        write: 0,
        len: 2,
        times: vec![0, 1, 2],
        scores: vec![None, Some(1), Some(2)],
    };
    for change in 0..8 {
        let mut state = base.clone();
        match change {
            0 => state.read = 3,
            1 => state.write = 3,
            2 => state.len = 4,
            3 => state.len = 1,
            4 => {
                state.scores.pop();
            }
            5 => state.scores[2] = None,
            6 => state.times[2] = 0,
            _ => {
                state.times.clear();
                state.scores.clear();
                state.len = 0;
                state.read = 0;
                state.write = 0;
            }
        }
        assert_eq!(
            TimedScoreRing::from_state(state).unwrap_err(),
            TimedScoreRingError::InvalidState
        );
    }
    let mut ring = TimedScoreRing::from_state(base.clone()).unwrap();
    ring.grow().unwrap();
    assert_eq!(ring.front(), Some((1, &1)));
    assert_eq!(ring.back(), Some((2, &2)));
    assert_eq!(ring.state().times, [1, 2, 0, 0, 0, 0]);
    assert_eq!(base.times, [0, 1, 2]);
}
