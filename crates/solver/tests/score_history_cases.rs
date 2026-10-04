// Adapted from pinned Timefold Community LateAcceptanceScoreBufferTest.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
use timetuck::{ScoreHistory, SimpleScore};
fn score(value: i64) -> SimpleScore {
    SimpleScore::new(value)
}
fn initial() -> ScoreHistory<SimpleScore> {
    ScoreHistory::new(3, score(-1000)).unwrap()
}
fn all(h: &ScoreHistory<SimpleScore>, value: i64) {
    for i in 0..3 {
        assert_eq!(h.get(i), Some(&score(value)));
    }
}
#[test]
fn initial_state_returns_initial_score() {
    let h = initial();
    all(&h, -1000);
    assert_eq!(h.current(), &score(-1000));
}
#[test]
fn update_writes_current_slot_and_advances_cursor() {
    let mut h = initial();
    h.update(score(-500));
    assert_eq!(h.get(0), Some(&score(-500)));
    assert_eq!(h.get(1), Some(&score(-1000)));
    assert_eq!(h.get(2), Some(&score(-1000)));
    assert_eq!(h.current(), &score(-1000));
}
#[test]
fn circular_buffer_wraps_around() {
    let mut h = initial();
    for v in [-100, -200, -300] {
        h.update(score(v));
    }
    assert_eq!(h.current(), &score(-100));
    h.update(score(-400));
    assert_eq!(h.get(0), Some(&score(-400)));
    assert_eq!(h.current(), &score(-200));
}
#[test]
fn try_reset_lazily_sets_all_slots() {
    let mut h = initial();
    h.update(score(-500));
    h.reset(score(-200));
    all(&h, -200);
}
#[test]
fn slot_written_after_reset_retains_own_value() {
    let mut h = initial();
    h.update(score(-1000));
    h.reset(score(0));
    h.update(score(-50));
    assert_eq!(h.get(0), Some(&score(0)));
    assert_eq!(h.get(1), Some(&score(-50)));
    assert_eq!(h.get(2), Some(&score(0)));
}
#[test]
fn try_reset_skipped_when_no_writes_and_same_score() {
    let mut h = initial();
    h.reset(score(0));
    all(&h, 0);
    h.reset(score(0));
    all(&h, 0);
}
#[test]
fn try_reset_fires_when_written_since_reset_even_with_same_score() {
    let mut h = initial();
    h.reset(score(0));
    h.update(score(-1000));
    h.reset(score(0));
    all(&h, 0);
}
