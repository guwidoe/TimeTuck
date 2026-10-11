// Adapted from pinned Timefold Community TerminationConfigTest and
// TerminationFactoryTest. SPDX-License-Identifier: Apache-2.0
use std::time::Duration;
use timetuck::{
    SolverLimits, TerminationProfile as P, TimeLimitSpec as Spec, TimeLimitUnits as Units,
};
fn spent(units: Units) -> P {
    P::default()
        .with_spent_time(Some(Spec::Units(units)))
        .unwrap()
}
fn seconds(n: u64) -> P {
    spent(Units {
        seconds: Some(n),
        ..Default::default()
    })
}
fn minutes(n: u64) -> P {
    spent(Units {
        minutes: Some(n),
        ..Default::default()
    })
}
#[test]
fn overwrite_spent_limit() {
    let p = minutes(1);
    assert!(
        p.spent_time_spec()
            .unwrap()
            .units()
            .unwrap()
            .minutes
            .is_some()
    );
    let p = p
        .with_spent_time(Some(Spec::Duration(Duration::from_secs(7200))))
        .unwrap();
    assert_eq!(
        p.spent_time_spec()
            .and_then(Spec::units)
            .and_then(|u| u.minutes),
        None
    );
    assert_eq!(p.spent_limit(), Some(Duration::from_secs(7200)));
}
#[test]
fn overwrite_unimproved_spent_limit() {
    let p = P::default()
        .with_unimproved_time(Some(Spec::Units(Units {
            minutes: Some(1),
            ..Default::default()
        })))
        .unwrap();
    assert!(
        p.unimproved_time_spec()
            .unwrap()
            .units()
            .unwrap()
            .minutes
            .is_some()
    );
    let p = p
        .with_unimproved_time(Some(Spec::Duration(Duration::from_secs(7200))))
        .unwrap();
    assert_eq!(
        p.unimproved_time_spec()
            .and_then(Spec::units)
            .and_then(|u| u.minutes),
        None
    );
    assert_eq!(p.unimproved_spent_limit(), Some(Duration::from_secs(7200)));
}
#[test]
fn child_without_time_spent_limit_inherits_parent() {
    let child = P::default();
    let parent = seconds(1);
    let merged = child.with_fallback(Some(&parent)).unwrap();
    assert_eq!(
        merged.spent_time_spec().unwrap().units().unwrap().seconds,
        Some(1)
    );
}
#[test]
fn child_seconds_override_parent_seconds() {
    let child = seconds(2);
    let parent = seconds(1);
    let merged = child.with_fallback(Some(&parent)).unwrap();
    assert_eq!(
        merged.spent_time_spec().unwrap().units().unwrap().seconds,
        Some(2)
    );
}
#[test]
fn child_time_group_overrides_parent_time_group() {
    let child = seconds(2);
    let parent = minutes(1);
    let merged = child.with_fallback(Some(&parent)).unwrap();
    let units = merged.spent_time_spec().unwrap().units().unwrap();
    assert_eq!(units.seconds, Some(2));
    assert_eq!(units.minutes, None);
}
#[test]
fn check_move_count_metrics() {
    let parent = P::new(None, None, None, Some(2), None, None).unwrap();
    let child = P::default();
    let merged = child.with_fallback(Some(&parent)).unwrap();
    assert_eq!(merged.move_count_limit(), Some(2));
    assert!(parent.is_configured());
}
fn duration() -> Duration {
    Duration::from_millis(5)
        + Duration::from_secs(4)
        + Duration::from_secs(3 * 60)
        + Duration::from_secs(2 * 3600)
        + Duration::from_secs(86400)
}
fn units() -> Units {
    Units {
        milliseconds: Some(5),
        seconds: Some(4),
        minutes: Some(3),
        hours: Some(2),
        days: Some(1),
    }
}
fn assert_bound(spec: Spec, unimproved: bool) {
    let p = if unimproved {
        P::default().with_unimproved_time(Some(spec))
    } else {
        P::default().with_spent_time(Some(spec))
    }
    .unwrap();
    p.with_limits(SolverLimits::<timetuck::SimpleScore>::default(), |limits| {
        // Distinct real runtime controls replace the Java termination class assertion.
        if unimproved {
            assert_eq!(limits.max_time, None);
            assert_eq!(limits.max_unimproved_time.unwrap().as_millis(), 93784005);
        } else {
            assert_eq!(limits.max_unimproved_time, None);
            assert_eq!(limits.max_time.unwrap().as_millis(), 93784005);
        }
    });
}
#[test]
fn spent_limit() {
    assert_bound(Spec::Duration(duration()), false);
}
#[test]
fn spent_limit_without_java_time() {
    assert_bound(Spec::Units(units()), false);
}
#[test]
fn unimproved_spent_limit() {
    assert_bound(Spec::Duration(duration()), true);
}
#[test]
fn unimproved_spent_limit_without_java_time() {
    assert_bound(Spec::Units(units()), true);
}
