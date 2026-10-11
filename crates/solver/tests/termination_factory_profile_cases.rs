// Adapted from pinned Timefold Community TerminationFactoryTest.
// SPDX-License-Identifier: Apache-2.0
use std::{borrow::Cow, ptr, time::Duration};
use timetuck::*;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn time_profile() -> TerminationProfile {
    TerminationProfile::default()
        .with_spent_time(Some(TimeLimitSpec::Units(TimeLimitUnits {
            milliseconds: Some(1000),
            ..Default::default()
        })))
        .unwrap()
        .with_unimproved_time(Some(TimeLimitSpec::Units(TimeLimitUnits {
            milliseconds: Some(1000),
            ..Default::default()
        })))
        .unwrap()
}
#[test]
fn build_termination_from_list_single() {
    let children = [SearchTermination::Condition(SearchLimit::Time(ms(1000)))];
    let result = SearchTermination::compose(&children, Default::default()).unwrap();
    assert!(matches!(result, Cow::Borrowed(_)));
    assert!(ptr::eq(result.as_ref(), &children[0]));
}
fn compose(style: TerminationComposition) {
    let children = [
        SearchTermination::Condition(SearchLimit::Time(ms(1000))),
        SearchTermination::Condition(SearchLimit::Steps(5)),
    ];
    let result = SearchTermination::compose(&children, style).unwrap();
    let actual = match (style, result.as_ref()) {
        (TerminationComposition::All, SearchTermination::All(children))
        | (TerminationComposition::Any, SearchTermination::Any(children)) => *children,
        _ => panic!("composition kind must match"),
    };
    assert_eq!(actual, children);
    // Preserve exact ordered child identities as well as values.
    assert!(ptr::eq(actual, children.as_slice()));
}
#[test]
fn build_termination_from_list_with_and_composition() {
    compose(TerminationComposition::All);
}
#[test]
fn build_termination_from_list_with_default_composition() {
    compose(Default::default());
}
#[test]
fn build_time_based_termination_with_score_difference_threshold() {
    let p = ScoredTerminationProfile::new(
        time_profile(),
        Some("1hard/0soft".parse::<HardSoftScore>().unwrap()),
        None,
    )
    .unwrap();
    p.with_limits(SolverLimits::default(), |limits| {
        assert_eq!(limits.max_time, Some(ms(1000)));
        assert_eq!(limits.max_unimproved_time, None);
        let significant = limits.significant_improvement.unwrap();
        assert_eq!(significant.window(), ms(1000));
        assert_eq!(significant.minimum(), &HardSoftScore::new(1, 0));
        assert_eq!(limits.max_move_evaluations, None);
    });
}
#[test]
fn build_time_based_termination_without_score_difference_threshold() {
    let p = ScoredTerminationProfile::<SimpleScore>::new(time_profile(), None, None).unwrap();
    p.with_limits(SolverLimits::default(), |limits| {
        assert_eq!(limits.max_time, Some(ms(1000)));
        assert_eq!(limits.max_unimproved_time, Some(ms(1000)));
        assert_eq!(limits.significant_improvement, None);
        assert_eq!(limits.max_move_evaluations, None);
    });
}
#[test]
fn build_with_move_count() {
    let profile = TerminationProfile::new(None, None, None, Some(1), None, None).unwrap();
    let p = ScoredTerminationProfile::<SimpleScore>::new(profile, None, None).unwrap();
    p.with_limits(SolverLimits::default(), |limits| {
        assert_eq!(limits.max_move_evaluations, Some(1));
        assert_eq!(limits.max_time, None);
        assert_eq!(limits.max_unimproved_time, None);
        assert_eq!(limits.significant_improvement, None);
    });
}
#[test]
fn score_difference_threshold_must_be_used_with_unimproved_time_spent() {
    let error = ScoredTerminationProfile::new(
        TerminationProfile::default(),
        Some("1hard/0soft".parse::<HardSoftScore>().unwrap()),
        None,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ScoredTerminationProfileError::MissingUnimprovedTime
    ));
    assert!(
        error
            .to_string()
            .contains("can only be used if an unimproved*SpentLimit")
    );
}
#[test]
fn best_score_feasible_requires_at_least_one_feasible_level() {
    let error = ScoredTerminationProfile::<SimpleScore>::new(
        TerminationProfile::default(),
        None,
        Some(true),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ScoredTerminationProfileError::NoFeasibleLevels
    ));
    assert!(
        error
            .to_string()
            .contains("can only be used with a score type that has at least 1 feasible level")
    );
}
