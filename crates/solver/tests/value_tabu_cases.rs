// Adapted ValueTabuAcceptorTest methods, Apache-2.0; see compat/provenance.json.
use timetuck::{Acceptance, FadingTabuMemory, SimpleScore, ValueAcceptor};
#[test]
fn tabu_size() {
    let best = SimpleScore::new(0);
    let mut a: ValueAcceptor<_, usize, Option<i32>> = Acceptance::ValueTabu {
        tenure: 2,
        aspiration: true,
    }
    .start_with_values(best)
    .unwrap();
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(3)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(4)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2)]));
    a.step_ended(best, &[], &0, &[Some(1)]);
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(3)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(4)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2)]));
    a.step_ended(best, &[], &0, &[Some(2)]);
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(3)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(4)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2)]));
    a.step_ended(best, &[], &0, &[Some(4)]);
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(3)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(4)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2)]));
    a.step_ended(best, &[], &0, &[Some(3)]);
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(3)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(4)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2)]));
    a.step_ended(best, &[], &0, &[Some(1)]);
}
#[test]
fn multiple_values_per_step() {
    let best = SimpleScore::new(0);
    let mut a: ValueAcceptor<_, usize, Option<i32>> = Acceptance::ValueTabu {
        tenure: 2,
        aspiration: true,
    }
    .start_with_values(best)
    .unwrap();
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(3)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(4)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0), Some(1)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0), Some(2)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0), Some(3)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0), Some(4)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1), Some(2)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1), Some(3)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1), Some(4)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2), Some(3)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2), Some(4)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(3), Some(4)]));
    a.step_ended(best, &[], &0, &[Some(0), Some(2)]);
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(3)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(4)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0), Some(1)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0), Some(2)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0), Some(3)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0), Some(4)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1), Some(2)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1), Some(3)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1), Some(4)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2), Some(3)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2), Some(4)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(3), Some(4)]));
    a.step_ended(best, &[], &0, &[Some(1)]);
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(3)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(4)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0), Some(1)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0), Some(2)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0), Some(3)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0), Some(4)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1), Some(2)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1), Some(3)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1), Some(4)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2), Some(3)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2), Some(4)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(3), Some(4)]));
    a.step_ended(best, &[], &0, &[Some(3), Some(4)]);
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(3)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(4)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0), Some(1)]));
    assert!(a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0), Some(2)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0), Some(3)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(0), Some(4)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1), Some(2)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1), Some(3)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(1), Some(4)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2), Some(3)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(2), Some(4)]));
    assert!(!a.accepts(best, SimpleScore::new(0), &[], &0, &[Some(3), Some(4)]));
    a.step_ended(best, &[], &0, &[Some(0)]);
}
#[test]
fn aspiration() {
    let best = SimpleScore::new(-100);
    let mut a: ValueAcceptor<_, usize, Option<i32>> = Acceptance::ValueTabu {
        tenure: 2,
        aspiration: true,
    }
    .start_with_values(best)
    .unwrap();
    a.step_ended(best, &[], &0, &[Some(1)]);
    assert!(a.accepts(best, SimpleScore::new(-120), &[], &0, &[Some(0)]));
    assert!(a.accepts(best, SimpleScore::new(-20), &[], &0, &[Some(0)]));
    assert!(!a.accepts(best, SimpleScore::new(-120), &[], &0, &[Some(1)]));
    assert!(a.accepts(best, SimpleScore::new(-20), &[], &0, &[Some(1)]));
    assert!(!a.accepts(best, SimpleScore::new(-120), &[], &0, &[Some(0), Some(1)]));
    assert!(a.accepts(best, SimpleScore::new(-20), &[], &0, &[Some(0), Some(1)]));
    a.step_ended(best, &[], &0, &[Some(1)]);
}
#[test]
fn unassigned_planning_value() {
    let z = SimpleScore::ZERO;
    let mut a: ValueAcceptor<_, usize, Option<i32>> = Acceptance::ValueTabu {
        tenure: 2,
        aspiration: true,
    }
    .start_with_values(z)
    .unwrap();
    assert!(a.accepts(z, z, &[], &0, &[Some(0), None, Some(1)]));
    a.step_ended(z, &[], &0, &[Some(0), None, Some(1)]);
    for values in [[Some(0)], [Some(1)], [None]] {
        assert!(!a.accepts(z, z, &[], &0, &values));
    }
    a.step_ended(z, &[], &0, &[Some(0)]);
    a.step_ended(z, &[], &0, &[Some(1)]);
    assert!(!a.accepts(z, z, &[], &0, &[Some(0)]));
    assert!(!a.accepts(z, z, &[], &0, &[Some(1)]));
    assert!(a.accepts(z, z, &[], &0, &[None]));
}
#[test]
fn fading_tabu_size() {
    let z = SimpleScore::ZERO;
    let mut a = FadingTabuMemory::new(2, 4, false, 0).unwrap();
    let no_draw = || panic!("source expects no random draw");
    a.step_ended(&[1]);
    assert!(!a.accepts_with_draw(&[1], z, z, no_draw).unwrap());
    assert!(a.accepts_with_draw(&[0], z, z, no_draw).unwrap());
    a.step_ended(&[0]);
    assert!(!a.accepts_with_draw(&[1], z, z, no_draw).unwrap());
    a.step_ended(&[0]);
    for expected in [false, true, true] {
        let mut calls = 0;
        assert_eq!(
            a.accepts_with_draw(&[1], z, z, || {
                calls += 1;
                0.5
            })
            .unwrap(),
            expected
        );
        assert_eq!(calls, 1);
        a.step_ended(&[0]);
    }
    assert!(a.accepts_with_draw(&[1], z, z, no_draw).unwrap());
    a.step_ended(&[0]);
    assert!(a.accepts_with_draw(&[1], z, z, no_draw).unwrap());
}
