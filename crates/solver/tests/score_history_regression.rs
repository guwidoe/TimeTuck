//! Original native state, cost and failure checks; no upstream-method credit.
#[path = "support/owned_score.rs"]
mod owned_score;
use owned_score::{OwnedScore, fail_clone_at};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use timetuck::{AcceptanceError, LateAcceptance, ScoreHistory};

#[test]
fn generated_history_matches_eager_reference() {
    for size in [1, 2, 3, 7, 128] {
        let mut history = ScoreHistory::new(size, -10).unwrap();
        let mut eager = vec![-10; size];
        let mut cursor = 0;
        let mut seed = 481u64;
        for step in 0..2000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let value = ((seed >> 32) % 17) as i64 - 8;
            if seed.is_multiple_of(5) {
                history.reset(value);
                eager.fill(value);
            } else {
                history.update(value);
                eager[cursor] = value;
                cursor = (cursor + 1) % size;
            }
            assert_eq!(history.cursor(), cursor);
            assert_eq!(history.current(), &eager[cursor]);
            assert!(history.iter().eq(eager.iter()));
            assert!(history.iter().rev().eq(eager.iter().rev()));
            assert_eq!(history.get(size), None);
            if step % 9 == 0 {
                assert_eq!(history.as_slice(), eager);
            }
        }
    }
}
#[test]
fn rejects_empty_and_impossible_capacity() {
    assert!(matches!(
        ScoreHistory::new(0, 0),
        Err(AcceptanceError::EmptyHistory)
    ));
    assert!(matches!(
        ScoreHistory::new(usize::MAX, 0),
        Err(AcceptanceError::HistoryAllocation)
    ));
}
#[derive(Debug)]
struct Counted {
    value: i64,
    clones: Arc<AtomicUsize>,
}
impl Clone for Counted {
    fn clone(&self) -> Self {
        self.clones.fetch_add(1, Ordering::SeqCst);
        Self {
            value: self.value,
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Counted {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for Counted {}
#[test]
fn large_history_clones_only_for_explicit_contiguous_observation() {
    let clones = Arc::new(AtomicUsize::new(0));
    let value = |v| Counted {
        value: v,
        clones: clones.clone(),
    };
    let mut h = ScoreHistory::new(100_000, value(0)).unwrap();
    for v in 1..50 {
        h.update(value(v));
        h.reset(value(-v));
    }
    assert_eq!(h.current().value, -49);
    assert!(h.iter().all(|v| v.value == -49));
    assert_eq!(clones.load(Ordering::SeqCst), 0);
    assert_eq!(h.as_slice().len(), 100_000);
    assert_eq!(h.as_slice()[49].value, -49);
    assert_eq!(clones.load(Ordering::SeqCst), 100_000);
    assert!(!h.reset(value(-49))); // No-op retains the cached observation.
    h.as_slice();
    assert_eq!(clones.load(Ordering::SeqCst), 100_000);
    h.update(value(8));
    assert_eq!(h.as_slice()[49].value, 8);
    assert_eq!(clones.load(Ordering::SeqCst), 200_000);
    h.reset(value(7));
    assert!(h.as_slice().iter().all(|v| v.value == 7));
    assert_eq!(clones.load(Ordering::SeqCst), 300_000);
}
#[test]
fn shared_observation_builds_one_snapshot() {
    let clones = Arc::new(AtomicUsize::new(0));
    let h = Arc::new(
        ScoreHistory::new(
            1000,
            Counted {
                value: 9,
                clones: clones.clone(),
            },
        )
        .unwrap(),
    );
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let h = h.clone();
            scope.spawn(move || assert!(h.as_slice().iter().all(|v| v.value == 9)));
        }
    });
    assert_eq!(clones.load(Ordering::SeqCst), 1000);
}
#[test]
fn cloned_history_and_cached_snapshot_are_independent() {
    for cached in [false, true] {
        let mut a = ScoreHistory::new(3, String::from("initial")).unwrap();
        a.update("old".into());
        a.reset("reset".into());
        a.update("current".into());
        if cached {
            a.as_slice();
        }
        let mut b = a.clone();
        b.update("other".into());
        a.reset("new".into());
        assert_eq!(a.as_slice(), &["new", "new", "new"]);
        assert_eq!(b.as_slice(), &["reset", "current", "other"]);
    }
}
#[test]
fn snapshot_clone_panic_preserves_history_and_allows_retry() {
    let mut h = ScoreHistory::new(3, OwnedScore::new(-2, -10)).unwrap();
    h.update(OwnedScore::new(-2, -5));
    fail_clone_at(2);
    assert!(catch_unwind(AssertUnwindSafe(|| h.as_slice())).is_err());
    assert_eq!(h.cursor(), 1);
    assert_eq!(h.current(), &OwnedScore::new(-2, -10));
    assert_eq!(
        h.as_slice(),
        &[
            OwnedScore::new(-2, -5),
            OwnedScore::new(-2, -10),
            OwnedScore::new(-2, -10)
        ]
    );
}
#[test]
fn late_acceptance_clone_failures_preserve_history_cursor_and_best() {
    for clone in [1, 2] {
        let mut state = LateAcceptance::new(3, false, OwnedScore::new(-2, 0)).unwrap();
        state.step_ended(OwnedScore::new(-3, 0));
        let before = state.history().to_vec(); // Exercise cached snapshot preservation too.
        fail_clone_at(clone);
        assert!(
            catch_unwind(AssertUnwindSafe(|| state.step_ended(OwnedScore::new(-1, 0)))).is_err()
        );
        assert_eq!(state.history(), before);
        assert_eq!(state.threshold(), OwnedScore::new(-2, 0));
        // If best was prematurely changed to -1, this improvement would not reset.
        state.step_ended(OwnedScore::new(-1, -1));
        assert!(
            state
                .history()
                .iter()
                .all(|s| *s == OwnedScore::new(-1, -1))
        );
        state.step_ended(OwnedScore::new(-4, 0));
        assert_eq!(state.history()[2], OwnedScore::new(-4, 0));
        assert_eq!(state.threshold(), OwnedScore::new(-1, -1));
    }
}
#[test]
fn late_acceptance_initialization_and_reset_clone_cost_do_not_scale_with_capacity() {
    // A third clone would panic: initialization uses one, a hard reset uses two.
    fail_clone_at(2);
    let mut state = LateAcceptance::new(100_000, false, OwnedScore::new(-2, 0)).unwrap();
    fail_clone_at(3);
    state.step_ended(OwnedScore::new(-1, 0));
    fail_clone_at(0);
    assert_eq!(state.threshold(), OwnedScore::new(-1, 0));
    assert!(state.accepts(OwnedScore::new(-2, 0), OwnedScore::new(-1, 0)));
}

#[test]
fn reset_equality_panic_preserves_cached_observation_and_cursor() {
    #[derive(Clone, Debug)]
    struct FallibleEq(i64, Arc<std::sync::atomic::AtomicBool>);
    impl PartialEq for FallibleEq {
        fn eq(&self, other: &Self) -> bool {
            assert!(
                !self.1.swap(false, Ordering::SeqCst),
                "injected equality failure"
            );
            self.0 == other.0
        }
    }
    impl Eq for FallibleEq {}
    let fail = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let value = |v| FallibleEq(v, fail.clone());
    let mut h = ScoreHistory::new(3, value(0)).unwrap();
    h.update(value(1));
    h.reset(value(2));
    let cached = h.as_slice().as_ptr();
    fail.store(true, Ordering::SeqCst);
    assert!(catch_unwind(AssertUnwindSafe(|| h.reset(value(3)))).is_err());
    assert_eq!(h.as_slice().as_ptr(), cached);
    assert_eq!(h.cursor(), 1);
    assert!(h.iter().all(|s| s.0 == 2));
    assert!(h.reset(value(3)));
    assert_eq!(h.cursor(), 1);
    assert!(h.iter().all(|s| s.0 == 3));
}
