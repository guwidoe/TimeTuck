// Adapted MoveTabuAcceptorTest methods; Apache-2.0, see compat/provenance.json.
use timetuck::{Acceptance, FadingTabuMemory, SimpleScore};
#[test]
fn tabu_size() {
    let zero = SimpleScore::ZERO;
    let mut a = Acceptance::MoveTabu {
        tenure: 2,
        aspiration: true,
    }
    .start_with_moves(zero)
    .unwrap();
    // The five source mock moves become stable distinct integer identities.
    for (blocked, selected) in [
        (vec![], 1),
        (vec![1], 2),
        (vec![1, 2], 3),
        (vec![2, 3], 4),
        (vec![3, 4], 0),
    ] {
        for movement in 0..5 {
            assert_eq!(
                a.accepts(zero, zero, &[], &movement),
                !blocked.contains(&movement)
            );
        }
        a.step_ended(zero, &[], &selected);
    }
}
#[test]
fn aspiration() {
    let best = SimpleScore::new(-100);
    let mut a = Acceptance::MoveTabu {
        tenure: 2,
        aspiration: true,
    }
    .start_with_moves(best)
    .unwrap();
    a.step_ended(best, &[], &1);
    for (movement, candidate, expected) in [
        (0, -120, true),
        (0, -20, true),
        (1, -120, false),
        (1, -20, true),
    ] {
        assert_eq!(
            a.accepts(best, SimpleScore::new(candidate), &[], &movement),
            expected
        );
    }
}
#[test]
fn fading_tabu_size() {
    let zero = SimpleScore::ZERO;
    let mut a = FadingTabuMemory::new(2, 4, false, 0).unwrap();
    let no_draw = || panic!("source requires no random draw");
    a.step_ended(&[1]);
    assert!(!a.accepts_with_draw(&[1], zero, zero, no_draw).unwrap());
    assert!(a.accepts_with_draw(&[0], zero, zero, no_draw).unwrap());
    a.step_ended(&[0]);
    assert!(!a.accepts_with_draw(&[1], zero, zero, no_draw).unwrap());
    a.step_ended(&[0]);
    for (draw, expected) in [(0.3, true), (0.5, true), (0.9, false)] {
        let mut calls = 0;
        assert_eq!(
            a.accepts_with_draw(&[1], zero, zero, || {
                calls += 1;
                draw
            })
            .unwrap(),
            expected
        );
        assert_eq!(calls, 1);
        a.step_ended(&[0]);
    }
    assert!(a.accepts_with_draw(&[1], zero, zero, no_draw).unwrap());
    a.step_ended(&[0]);
    assert!(a.accepts_with_draw(&[1], zero, zero, no_draw).unwrap());
}
