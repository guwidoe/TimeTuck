// Adapted from Timefold Community 2.7.0 EntityTabuAcceptorTest.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
// Native stable variable IDs replace Java test entities; all assertions preserved.
use timetuck::{SimpleScore, TabuMemory};

#[test]
fn tabu_size() {
    let mut memory = TabuMemory::new(2, true).unwrap();
    let best = SimpleScore::new(0);
    assert!(memory.accepts(&[0], SimpleScore::new(0), best));
    assert!(memory.accepts(&[1], SimpleScore::new(0), best));
    assert!(memory.accepts(&[2], SimpleScore::new(0), best));
    assert!(memory.accepts(&[3], SimpleScore::new(0), best));
    assert!(memory.accepts(&[4], SimpleScore::new(0), best));
    assert!(memory.accepts(&[2], SimpleScore::new(0), best));
    memory.step_ended(&[1]);
    assert!(memory.accepts(&[0], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[1], SimpleScore::new(0), best));
    assert!(memory.accepts(&[2], SimpleScore::new(0), best));
    assert!(memory.accepts(&[3], SimpleScore::new(0), best));
    assert!(memory.accepts(&[4], SimpleScore::new(0), best));
    assert!(memory.accepts(&[2], SimpleScore::new(0), best));
    memory.step_ended(&[2]);
    assert!(memory.accepts(&[0], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[1], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[2], SimpleScore::new(0), best));
    assert!(memory.accepts(&[3], SimpleScore::new(0), best));
    assert!(memory.accepts(&[4], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[2], SimpleScore::new(0), best));
    memory.step_ended(&[4]);
    assert!(memory.accepts(&[0], SimpleScore::new(0), best));
    assert!(memory.accepts(&[1], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[2], SimpleScore::new(0), best));
    assert!(memory.accepts(&[3], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[4], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[2], SimpleScore::new(0), best));
    memory.step_ended(&[3]);
    assert!(memory.accepts(&[0], SimpleScore::new(0), best));
    assert!(memory.accepts(&[1], SimpleScore::new(0), best));
    assert!(memory.accepts(&[2], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[3], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[4], SimpleScore::new(0), best));
    assert!(memory.accepts(&[2], SimpleScore::new(0), best));
    memory.step_ended(&[1]);
}

#[test]
fn tabu_size_multiple_entities_per_step() {
    let mut memory = TabuMemory::new(2, true).unwrap();
    let best = SimpleScore::new(0);
    assert!(memory.accepts(&[0], SimpleScore::new(0), best));
    assert!(memory.accepts(&[1], SimpleScore::new(0), best));
    assert!(memory.accepts(&[2], SimpleScore::new(0), best));
    assert!(memory.accepts(&[3], SimpleScore::new(0), best));
    assert!(memory.accepts(&[4], SimpleScore::new(0), best));
    assert!(memory.accepts(&[0, 1], SimpleScore::new(0), best));
    assert!(memory.accepts(&[0, 2], SimpleScore::new(0), best));
    assert!(memory.accepts(&[0, 3], SimpleScore::new(0), best));
    assert!(memory.accepts(&[0, 4], SimpleScore::new(0), best));
    assert!(memory.accepts(&[1, 2], SimpleScore::new(0), best));
    assert!(memory.accepts(&[1, 3], SimpleScore::new(0), best));
    assert!(memory.accepts(&[1, 4], SimpleScore::new(0), best));
    assert!(memory.accepts(&[2, 3], SimpleScore::new(0), best));
    assert!(memory.accepts(&[2, 4], SimpleScore::new(0), best));
    assert!(memory.accepts(&[3, 4], SimpleScore::new(0), best));
    memory.step_ended(&[0, 2]);
    assert!(!memory.accepts(&[0], SimpleScore::new(0), best));
    assert!(memory.accepts(&[1], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[2], SimpleScore::new(0), best));
    assert!(memory.accepts(&[3], SimpleScore::new(0), best));
    assert!(memory.accepts(&[4], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[0, 1], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[0, 2], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[0, 3], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[0, 4], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[1, 2], SimpleScore::new(0), best));
    assert!(memory.accepts(&[1, 3], SimpleScore::new(0), best));
    assert!(memory.accepts(&[1, 4], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[2, 3], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[2, 4], SimpleScore::new(0), best));
    assert!(memory.accepts(&[3, 4], SimpleScore::new(0), best));
    memory.step_ended(&[1]);
    assert!(!memory.accepts(&[0], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[1], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[2], SimpleScore::new(0), best));
    assert!(memory.accepts(&[3], SimpleScore::new(0), best));
    assert!(memory.accepts(&[4], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[0, 1], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[0, 2], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[0, 3], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[0, 4], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[1, 2], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[1, 3], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[1, 4], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[2, 3], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[2, 4], SimpleScore::new(0), best));
    assert!(memory.accepts(&[3, 4], SimpleScore::new(0), best));
    memory.step_ended(&[3, 4]);
    assert!(memory.accepts(&[0], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[1], SimpleScore::new(0), best));
    assert!(memory.accepts(&[2], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[3], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[4], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[0, 1], SimpleScore::new(0), best));
    assert!(memory.accepts(&[0, 2], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[0, 3], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[0, 4], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[1, 2], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[1, 3], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[1, 4], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[2, 3], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[2, 4], SimpleScore::new(0), best));
    assert!(!memory.accepts(&[3, 4], SimpleScore::new(0), best));
    memory.step_ended(&[0]);
}

#[test]
fn aspiration() {
    let mut memory = TabuMemory::new(2, true).unwrap();
    let best = SimpleScore::new(-100);
    memory.step_ended(&[1]);
    assert!(memory.accepts(&[0], SimpleScore::new(-120), best));
    assert!(memory.accepts(&[0], SimpleScore::new(-20), best));
    assert!(!memory.accepts(&[1], SimpleScore::new(-120), best));
    assert!(memory.accepts(&[1], SimpleScore::new(-20), best));
    assert!(!memory.accepts(&[0, 1], SimpleScore::new(-120), best));
    assert!(memory.accepts(&[0, 1], SimpleScore::new(-20), best));
    memory.step_ended(&[1]);
}
