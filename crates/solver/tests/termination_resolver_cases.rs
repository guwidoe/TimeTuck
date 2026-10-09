// Adapted from pinned Timefold Community worker TerminationServiceTest.
// SPDX-License-Identifier: Apache-2.0
use std::time::Duration;
use timetuck::*;
fn sec(n: u64) -> Option<Duration> {
    Some(Duration::from_secs(n))
}
fn service() -> TerminationResolver {
    TerminationResolver::new(Duration::from_secs(10), None, None, None).unwrap()
}
fn request(
    spent: Option<Duration>,
    unimproved: Option<Duration>,
    steps: Option<i64>,
    moves: Option<i64>,
    window: Option<Duration>,
    ratio: Option<(i64, u64)>,
) -> TerminationProfile {
    TerminationProfile::new(spent, unimproved, steps, moves, window, ratio).unwrap()
}
fn error(result: Result<TerminationResolver, TerminationProfileError>, parts: &[&str]) {
    let e = result.unwrap_err().to_string();
    for part in parts {
        assert!(e.contains(part), "{e}: {part}");
    }
}
#[test]
fn default_spent_and_slowdown() {
    let r = service().resolve(None);
    let p = r.profile();
    assert_eq!(p.spent_limit(), sec(10));
    assert_eq!(p.unimproved_spent_limit(), None);
    assert_eq!(p.step_count_limit(), None);
    assert_eq!(p.move_count_limit(), None);
    assert!(r.uses_diminished_returns());
    assert_eq!(p.sliding_window(), None);
    assert_eq!(p.minimum_improvement_ratio(), None);
    // Native budgets are independent OR stops, without an enclosing All expression.
    r.with_sequence(SolverConfig::<SimpleScore>::default(), |plan| {
        assert_eq!(plan.limits.max_time, sec(10));
        assert!(plan.limits.termination.is_none());
        let SolverPhase::LocalSearch(local) = &plan.phases[1] else {
            panic!()
        };
        assert!(local.diminished_returns.is_some());
    });
}
#[test]
fn request_tuning_forwarded() {
    let q = request(sec(60), None, None, None, sec(300), Some((1, 100)));
    let r = service().resolve(Some(&q));
    let p = r.profile();
    assert_eq!(p.spent_limit(), sec(60));
    assert_eq!(p.unimproved_spent_limit(), None);
    assert_eq!(p.step_count_limit(), None);
    assert_eq!(p.move_count_limit(), None);
    assert!(r.uses_diminished_returns());
    assert_eq!(p.sliding_window(), sec(300));
    assert_eq!(p.minimum_improvement_ratio(), Some((1, 100)));
}
#[test]
fn stagnation_disables_slowdown() {
    let q = request(sec(60), sec(30), None, None, sec(300), Some((1, 100)));
    let r = service().resolve(Some(&q));
    assert_eq!(r.profile().unimproved_spent_limit(), sec(30));
    assert_eq!(r.profile().step_count_limit(), None);
    assert_eq!(r.profile().move_count_limit(), None);
    assert!(!r.uses_diminished_returns());
}
#[test]
fn steps_disable_slowdown() {
    let q = request(sec(60), None, Some(1000), None, sec(300), Some((1, 100)));
    let r = service().resolve(Some(&q));
    assert_eq!(r.profile().step_count_limit(), Some(1000));
    assert_eq!(r.profile().move_count_limit(), None);
    assert_eq!(r.profile().unimproved_spent_limit(), None);
    assert!(!r.uses_diminished_returns());
}
#[test]
fn moves_disable_slowdown() {
    let q = request(sec(60), None, None, Some(100000), sec(300), Some((1, 100)));
    let r = service().resolve(Some(&q));
    assert_eq!(r.profile().move_count_limit(), Some(100000));
    assert_eq!(r.profile().step_count_limit(), None);
    assert_eq!(r.profile().unimproved_spent_limit(), None);
    assert!(!r.uses_diminished_returns());
}
#[test]
fn counts_combine() {
    let q = request(sec(60), None, Some(1000), Some(100000), None, None);
    let r = service().resolve(Some(&q));
    assert_eq!(r.profile().step_count_limit(), Some(1000));
    assert_eq!(r.profile().move_count_limit(), Some(100000));
    assert_eq!(r.profile().unimproved_spent_limit(), None);
    assert!(!r.uses_diminished_returns());
}
#[test]
fn missing_request_time_inherits() {
    let q = request(None, None, None, None, None, None);
    let r = service().resolve(Some(&q));
    assert_eq!(r.profile().spent_limit(), sec(10));
    assert!(r.uses_diminished_returns());
}
#[test]
fn platform_stagnation_disables_slowdown() {
    let s = TerminationResolver::new(Duration::from_secs(10), sec(5), None, None).unwrap();
    let r = s.resolve(None);
    assert_eq!(r.profile().unimproved_spent_limit(), sec(5));
    assert!(!r.uses_diminished_returns());
}
#[test]
fn platform_steps_disable_slowdown() {
    let s = TerminationResolver::new(Duration::from_secs(10), None, Some(50), None).unwrap();
    let r = s.resolve(None);
    assert_eq!(r.profile().step_count_limit(), Some(50));
    assert!(!r.uses_diminished_returns());
}
#[test]
fn platform_moves_disable_slowdown() {
    let s = TerminationResolver::new(Duration::from_secs(10), None, None, Some(500000)).unwrap();
    let r = s.resolve(None);
    assert_eq!(r.profile().move_count_limit(), Some(500000));
    assert!(!r.uses_diminished_returns());
}
#[test]
fn rejects_negative_platform_steps() {
    error(
        TerminationResolver::new(Duration::from_secs(10), None, Some(-1), None),
        &["stepCountLimit", "cannot be negative"],
    );
}
#[test]
fn rejects_negative_platform_moves() {
    error(
        TerminationResolver::new(Duration::from_secs(10), None, None, Some(-1)),
        &["moveCountLimit", "cannot be negative"],
    );
}
#[test]
fn zero_platform_counts() {
    let s = TerminationResolver::new(Duration::from_secs(10), None, Some(0), Some(0)).unwrap();
    let r = s.resolve(None);
    assert_eq!(r.profile().step_count_limit(), Some(0));
    assert_eq!(r.profile().move_count_limit(), Some(0));
    assert!(!r.uses_diminished_returns());
}
#[test]
fn rejects_platform_stagnation_and_steps() {
    error(
        TerminationResolver::new(Duration::from_secs(10), sec(5), Some(1000), None),
        &["unimprovedSpentLimit", "stepCountLimit"],
    );
}
#[test]
fn rejects_platform_stagnation_and_moves() {
    error(
        TerminationResolver::new(Duration::from_secs(10), sec(5), None, Some(100000)),
        &["unimprovedSpentLimit", "moveCountLimit"],
    );
}
#[test]
fn request_steps_ignore_platform_stagnation() {
    let s = TerminationResolver::new(Duration::from_secs(10), sec(5), None, None).unwrap();
    let q = request(None, None, Some(1000), None, None, None);
    let r = s.resolve(Some(&q));
    assert_eq!(r.profile().step_count_limit(), Some(1000));
    assert_eq!(r.profile().unimproved_spent_limit(), None);
}
#[test]
fn request_moves_ignore_platform_stagnation() {
    let s = TerminationResolver::new(Duration::from_secs(10), sec(5), None, None).unwrap();
    let q = request(None, None, None, Some(100000), None, None);
    let r = s.resolve(Some(&q));
    assert_eq!(r.profile().move_count_limit(), Some(100000));
    assert_eq!(r.profile().unimproved_spent_limit(), None);
}
#[test]
fn request_stagnation_ignores_platform_counts() {
    let s =
        TerminationResolver::new(Duration::from_secs(10), None, Some(50), Some(500000)).unwrap();
    let q = request(None, sec(30), None, None, None, None);
    let r = s.resolve(Some(&q));
    assert_eq!(r.profile().unimproved_spent_limit(), sec(30));
    assert_eq!(r.profile().step_count_limit(), None);
    assert_eq!(r.profile().move_count_limit(), None);
}
#[test]
fn request_steps_inherit_other_count() {
    let s = TerminationResolver::new(Duration::from_secs(10), None, None, Some(500000)).unwrap();
    let q = request(None, None, Some(1000), None, None, None);
    let r = s.resolve(Some(&q));
    assert_eq!(r.profile().step_count_limit(), Some(1000));
    assert_eq!(r.profile().move_count_limit(), Some(500000));
}
#[test]
fn request_moves_override_platform() {
    let s = TerminationResolver::new(Duration::from_secs(10), None, None, Some(500000)).unwrap();
    let q = request(None, None, None, Some(100000), None, None);
    let r = s.resolve(Some(&q));
    assert_eq!(r.profile().move_count_limit(), Some(100000));
}
