//! Original real search stopping and configuration publication controls.
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use timetuck::*;

#[test]
fn inner_time_policy_stops_actual_exact_search_with_controlled_clock() {
    let ticks = Arc::new(AtomicU64::new(0));
    let clock_ticks = ticks.clone();
    let clock = SolverClock::new(move || Duration::from_millis(clock_ticks.load(Ordering::SeqCst)));
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i32, 1]); 3]).unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("costly score", vec![0, 1, 2], |_| {
        ticks.fetch_add(1000, Ordering::SeqCst);
        SimpleScore::ZERO
    })
    .unwrap();
    let built = InnerTerminationConfig {
        children: vec![InnerTerminationCondition::SpentTime(TimeLimitSpec::Units(
            TimeLimitUnits {
                seconds: Some(1),
                ..Default::default()
            },
        ))],
        ..Default::default()
    }
    .build()
    .unwrap();
    let result = built.with_limits(SolverLimits::default(), |limits| {
        solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
                limits,
            },
            SolverExtensions {
                clock: Some(&clock),
                ..Default::default()
            },
        )
        .unwrap()
    });
    assert_eq!(result.work.evaluations, 1);
    assert!(!result.is_optimal());
    assert_eq!(result.termination, SolverTermination::Condition);
}

#[test]
fn ordered_and_children_keep_inherited_guard_and_empty_units_are_absent() {
    let built = InnerTerminationConfig {
        composition: TerminationComposition::All,
        children: vec![
            InnerTerminationCondition::SpentTime(TimeLimitSpec::Units(Default::default())),
            InnerTerminationCondition::Native(SearchLimit::Steps(5)),
            InnerTerminationCondition::SpentTime(TimeLimitSpec::Duration(Duration::ZERO)),
        ],
    }
    .build()
    .unwrap();
    assert_eq!(
        built.children(),
        &[
            SearchTermination::Condition(SearchLimit::Steps(5)),
            SearchTermination::Condition(SearchLimit::Time(Duration::ZERO)),
        ]
    );
    let inherited = SearchTermination::Condition(SearchLimit::Steps(2));
    let limits = SolverLimits::<SimpleScore> {
        termination: Some(&inherited),
        max_evaluations: 17,
        ..Default::default()
    };
    built.with_limits(limits, |actual| {
        assert_eq!(actual.max_evaluations, 17);
        assert!(
            actual
                .termination
                .unwrap()
                .status(SearchProgress {
                    completed_steps: 2,
                    ..Default::default()
                })
                .unwrap()
                .reached
        );
    });
    InnerTerminationConfig::default()
        .build()
        .unwrap()
        .with_limits(limits, |actual| assert_eq!(actual, limits));
}

#[test]
fn overflow_after_valid_child_does_not_publish_a_partial_policy() {
    let config = InnerTerminationConfig {
        children: vec![
            InnerTerminationCondition::Native(SearchLimit::Steps(1)),
            InnerTerminationCondition::SpentTime(TimeLimitSpec::Units(TimeLimitUnits {
                days: Some(u64::MAX),
                ..Default::default()
            })),
        ],
        ..Default::default()
    };
    let prior = config.clone();
    assert!(matches!(
        config.build(),
        Err(TerminationProfileError::TimeOverflow)
    ));
    assert_eq!(config, prior);
}
