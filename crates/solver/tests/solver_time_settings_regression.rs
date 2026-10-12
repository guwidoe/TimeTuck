//! Original controls for fluent settings consumed by actual solver execution.
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use timetuck::*;

#[test]
fn settings_preserve_other_limits_and_phase_plans_without_score_clone_bounds() {
    let duration = Duration::from_secs(120);
    macro_rules! check {
        ($ty:ident) => {{
            let original = $ty::<SimpleScore>::default();
            let changed = original
                .clone()
                .with_time_limit(Some(duration))
                .with_unimproved_time_limit(Some(Duration::ZERO));
            assert_eq!(changed.limits.max_time, Some(duration));
            assert_eq!(changed.limits.max_unimproved_time, Some(Duration::ZERO));
            assert_eq!(
                changed
                    .with_time_limit(None)
                    .with_unimproved_time_limit(None),
                original
            );
        }};
    }
    check!(SolverConfig);
    check!(SolverPlan);
    check!(SolverSequence);
    check!(MixedSolverConfig);
    check!(MixedSolverPlan);
    check!(MixedSolverSequence);
    struct NonCloneScore;
    let limits = SolverLimits::<NonCloneScore>::default()
        .with_time_limit(Some(duration))
        .with_unimproved_time_limit(Some(duration));
    assert_eq!(limits.max_time, Some(duration));
    assert_eq!(limits.max_unimproved_time, Some(duration));
}

fn execute(stream: bool, stagnation: bool) {
    let elapsed = Arc::new(AtomicU64::new(0));
    let ticks = elapsed.clone();
    let clock = SolverClock::new(move || Duration::from_secs(ticks.load(Ordering::Relaxed)));
    let problem =
        FiniteProblem::new(vec![Variable::new((0_i64..20).collect()).with_initial(0)]).unwrap();
    let config = SolverConfig::<SimpleScore>::default();
    let config = if stagnation {
        config.with_unimproved_time_limit(Some(Duration::from_secs(120)))
    } else {
        config.with_time_limit(Some(Duration::from_secs(120)))
    };
    let result = if stream {
        solve_stream_with_extensions(
            &problem,
            || {
                let mut builder = StreamGraphBuilder::new();
                let source = builder.source::<usize, (usize, Option<i64>)>();
                let tick = elapsed.clone();
                builder.score("clocked constant", source.port(), move |_| {
                    tick.fetch_add(30, Ordering::Relaxed);
                    SimpleScore::new(7)
                })?;
                Ok((builder.build(), source))
            },
            config,
            SolverExtensions {
                clock: Some(&clock),
                ..Default::default()
            },
        )
        .unwrap()
    } else {
        let mut network = ConstraintNetwork::new(1);
        let tick = elapsed.clone();
        network
            .add_constraint("clocked constant", vec![0], move |_| {
                tick.fetch_add(30, Ordering::Relaxed);
                SimpleScore::new(7)
            })
            .unwrap();
        solve_with_extensions(
            &problem,
            &network,
            config,
            SolverExtensions {
                clock: Some(&clock),
                ..Default::default()
            },
        )
        .unwrap()
    };
    assert_eq!(
        result.termination,
        if stagnation {
            SolverTermination::UnimprovedTimeLimit
        } else {
            SolverTermination::TimeLimit
        }
    );
    assert_eq!(result.score, Some(SimpleScore::new(7)));
    assert_eq!(result.assignment.values(), &[Some(0)]);
    assert!(elapsed.load(Ordering::Relaxed) >= 120);
}

#[test]
fn configured_two_minute_spent_limit_stops_real_dense_and_stream_solves() {
    execute(false, false);
    execute(true, false);
}
#[test]
fn configured_two_minute_stagnation_limit_stops_real_dense_and_stream_solves() {
    execute(false, true);
    execute(true, true);
}
