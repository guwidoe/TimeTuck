// Adapted from Timefold Community SolutionManagerTest.updateOnlyScoreFailsIfListVariableInconsistent.
// Copyright Timefold BV and contributors. Apache-2.0; see compat/provenance.json.
#[path = "support/concurrent_shadow_fixture.rs"]
mod fixture;
use std::cell::Cell;
use timetuck::*;
#[test]
fn update_only_score_fails_if_list_variable_inconsistent() {
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
                    owner: Some(1),
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
    let before = s.clone();
    let error = m
        .update_with_policy(&mut s, SolutionUpdatePolicy::ScoreOnly)
        .unwrap_err();
    let ShadowSolutionError::Adapter(ModelShadowError::Builtin(
        SolutionRefreshError::ShadowMismatch {
            value,
            mismatch: ListShadowMismatch::Owner { expected, actual },
        },
    )) = error
    else {
        panic!("expected wrong inverse owner: {error:?}");
    };
    assert_eq!(fixture::VALUES[value], "b1");
    assert_eq!(expected, Some(0)); // e1 owns [a1,b1].
    assert_eq!(actual, Some(1)); // b1 incorrectly points to e2.
    // Native value/owner identities retain the source diagnostic's meaningful
    // fields; list-variable and Java object-chain text are implicit in this API.
    assert_eq!(s, before);
    assert_eq!(calls.get(), 0);
}
