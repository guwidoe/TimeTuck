// Adapted from Timefold Community SolutionManagerTest.updateOnlyScoreDeclarativeShadows.
// Copyright Timefold BV and contributors. Apache-2.0; see compat/provenance.json.
#[path = "support/concurrent_shadow_fixture.rs"]
mod fixture;
use std::cell::Cell;
use timetuck::*;
#[test]
fn update_only_score_declarative_shadows() {
    let schema = fixture::schema();
    let p = fixture::problem();
    let calls = Cell::new(0);
    let fail = Cell::new(false);
    let mut m = fixture::manager(&schema, &p, &calls, &fail);
    let mut s = ManagedSolution::new(
        p.initial_assignment(),
        ListDerivedShadows {
            builtin: vec![
                ListBuiltinValues {
                    owner: Some(0),
                    index: Some(0),
                    previous: None,
                    next: Some("b1"),
                },
                ListBuiltinValues {
                    owner: Some(1),
                    index: Some(1),
                    previous: Some("b2"),
                    next: None,
                },
                ListBuiltinValues {
                    owner: Some(0),
                    index: Some(1),
                    previous: Some("a1"),
                    next: None,
                },
                ListBuiltinValues {
                    owner: Some(1),
                    index: Some(0),
                    previous: None,
                    next: Some("a2"),
                },
            ],
            derived: fixture::Schedule {
                inconsistent: [true; 4],
                ..Default::default()
            },
        },
    );
    assert_eq!(
        m.update_with_policy(&mut s, SolutionUpdatePolicy::ScoreOnly)
            .unwrap(),
        Some(&HardSoftScore::new(-4, 0))
    );
    assert_eq!(s.score, Some(HardSoftScore::new(-4, 0)));
    assert_eq!(
        calls.get(),
        0,
        "score-only must preserve supplied flags without graph evaluation"
    );
}
