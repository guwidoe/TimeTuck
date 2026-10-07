use std::time::Duration;
use timetuck::*;
#[test]
fn only_missing_local_policies_change_and_reapplication_is_idempotent() {
    let original = DiminishedReturnsConfig::new(Duration::from_secs(9), 1, 2).unwrap();
    let default = default_diminished_returns_config();
    let mut plan = SolverSequence::<SimpleScore> {
        phases: vec![
            SolverPhase::Construction(Default::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 7,
                ..Default::default()
            }),
            SolverPhase::Exact(Default::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                diminished_returns: Some(original),
                ..Default::default()
            }),
            SolverPhase::Custom(Default::default()),
            SolverPhase::RuinRecreate(Default::default()),
        ],
        ..Default::default()
    };
    let before = plan.clone();
    assert_eq!(apply_diminished_returns_default(&mut plan, default), 1);
    assert_eq!(apply_diminished_returns_default(&mut plan, default), 0);
    assert_eq!(plan.limits, before.limits);
    for index in [0, 2, 3, 4, 5] {
        assert_eq!(plan.phases[index], before.phases[index]);
    }
    let SolverPhase::LocalSearch(local) = &plan.phases[1] else {
        panic!()
    };
    assert_eq!(local.max_steps, 7);
    assert_eq!(local.diminished_returns, Some(default));
    plan.phases
        .push(SolverPhase::LocalSearch(Default::default()));
    assert_eq!(apply_diminished_returns_default(&mut plan, default), 1);
}
#[test]
fn mixed_configuration_keeps_overrides_and_does_not_require_clone_scores() {
    struct NonCloneScore;
    let explicit = DiminishedReturnsConfig::new(Duration::from_secs(4), 3, 4).unwrap();
    let mut plan = MixedSolverSequence::<NonCloneScore> {
        phases: vec![
            MixedSolverPhase::Construction(Default::default()),
            MixedSolverPhase::LocalSearch(Default::default()),
            MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    diminished_returns: Some(explicit),
                    ..Default::default()
                },
                ..Default::default()
            }),
            MixedSolverPhase::Exact(Default::default()),
        ],
        ..Default::default()
    };
    assert_eq!(
        apply_mixed_diminished_returns_default(&mut plan, default_diminished_returns_config()),
        1
    );
    assert_eq!(
        apply_mixed_diminished_returns_default(&mut plan, default_diminished_returns_config()),
        0
    );
    let MixedSolverPhase::LocalSearch(local) = &plan.phases[2] else {
        panic!()
    };
    assert_eq!(local.search.diminished_returns, Some(explicit));
    let mut plain = SolverSequence::<NonCloneScore>::default();
    assert_eq!(apply_diminished_returns_default(&mut plain, explicit), 1);
}
