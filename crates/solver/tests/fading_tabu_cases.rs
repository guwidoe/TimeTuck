// Adapted EntityTabuAcceptorTest.fadingTabuSize, Apache-2.0; see provenance.
use timetuck::{FadingTabuMemory, SimpleScore};
#[test]
fn entity_fading_tabu_size() {
    let mut memory = FadingTabuMemory::new(2, 4, false, 0).unwrap();
    let zero = SimpleScore::ZERO;
    memory.step_ended(&["e1"]);
    let no_draw = || panic!("the source expects no random draw");
    assert!(
        !memory
            .accepts_with_draw(&["e1"], zero, zero, no_draw)
            .unwrap()
    );
    assert!(
        memory
            .accepts_with_draw(&["e0"], zero, zero, no_draw)
            .unwrap()
    );
    memory.step_ended(&["e0"]);
    assert!(
        !memory
            .accepts_with_draw(&["e1"], zero, zero, no_draw)
            .unwrap()
    );
    memory.step_ended(&["e0"]);
    for (draw, accepted) in [(0.5, false), (0.5, true), (0.1, true)] {
        let mut calls = 0;
        assert_eq!(
            memory
                .accepts_with_draw(&["e1"], zero, zero, || {
                    calls += 1;
                    draw
                })
                .unwrap(),
            accepted
        );
        assert_eq!(calls, 1);
        memory.step_ended(&["e0"]);
    }
    assert!(
        memory
            .accepts_with_draw(&["e1"], zero, zero, no_draw)
            .unwrap()
    );
    memory.step_ended(&["e0"]);
    assert!(
        memory
            .accepts_with_draw(&["e1"], zero, zero, no_draw)
            .unwrap()
    );
}
