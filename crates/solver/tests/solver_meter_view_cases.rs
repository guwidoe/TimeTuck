// Adapted pinned Community SolverMetricsIT and TEST-only fixture/setup helpers.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use timetuck::*;
const PROBLEM_ID: &str = "1970-01-01T00:00Z";
const WAIT: Duration = Duration::from_secs(10);
fn model(n: usize) -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new((0..n as i32).collect()); n]).unwrap()
}
fn monitor(p: &FiniteProblem<i32>) -> SolverMetrics<SimpleScore> {
    SolverMetrics::with_clock(Some((&p.problem_size_statistics()).into()), || {
        Duration::ZERO
    })
}
fn configuration(token: Option<&CancellationToken>) -> SolverConfig<'_> {
    SolverConfig {
        limits: SolverLimits {
            cancellation: token,
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            max_steps: 10,
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn expected_id(name: &'static str, meter_type: SolverMeterType, problem: bool) -> SolverMeterId {
    SolverMeterId {
        name,
        meter_type,
        base_unit: None,
        description: None,
        tags: if problem {
            BTreeMap::from([("problem.id".into(), PROBLEM_ID.into())])
        } else {
            BTreeMap::new()
        },
    }
}
fn ids(active: bool) -> BTreeSet<SolverMeterId> {
    let mut out = BTreeSet::from([
        expected_id(
            "timefold.solver.solve.duration",
            SolverMeterType::LongTaskTimer,
            false,
        ),
        expected_id("timefold.solver.errors", SolverMeterType::Counter, false),
    ]);
    if active {
        for name in [
            "timefold.solver.score.calculation.count",
            "timefold.solver.move.evaluation.count",
            "timefold.solver.problem.entities",
            "timefold.solver.problem.variables",
            "timefold.solver.problem.values",
            "timefold.solver.problem.size.log",
        ] {
            out.insert(expected_id(name, SolverMeterType::Gauge, true));
        }
    }
    out
}
fn default_case() {
    // Preserve two uninitialized entities/two values, zero dummy score,
    // construction followed by at most ten local steps, and the epoch clock/id.
    let p = model(2);
    let metrics = monitor(&p);
    let mut view = SolverMeterView::for_problem(metrics.clone(), PROBLEM_ID, &p);
    assert!(view.publish_solver_meters().unwrap().is_empty());
    let mut observed = false;
    let mut events = |event: SolverEvent<'_, Assignment<i32>, SimpleScore>| {
        if let SolverEvent::Best(_) = event
            && !observed
        {
            let readings = view.publish_solver_meters().unwrap();
            assert_eq!(
                readings.into_iter().map(|r| r.id).collect::<BTreeSet<_>>(),
                ids(true)
            );
            observed = true;
        }
    };
    let result = solve_with_extensions(
        &p,
        &ConstraintNetwork::<_, SimpleScore>::new(2),
        configuration(None),
        SolverExtensions {
            metrics: Some(&metrics),
            events: Some(&mut events),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(observed);
    assert!(result.complete);
    assert_eq!(
        view.current_solver_meters()
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect::<BTreeSet<_>>(),
        ids(false)
    );
}
#[test]
fn check_default_meters() {
    default_case();
}
#[test]
fn check_default_meters_tags() {
    default_case();
}
#[test]
fn solve_metrics_problem_change() {
    let p = model(2);
    let initial_metrics = monitor(&p);
    let view = Arc::new(Mutex::new(SolverMeterView::for_problem(
        initial_metrics.clone(),
        PROBLEM_ID,
        &p,
    )));
    assert!(
        view.lock()
            .unwrap()
            .publish_solver_meters()
            .unwrap()
            .is_empty()
    );
    let shared = view.clone();
    let (started, start) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let mut manager = LiveSolverManager::<
        FiniteProblem<i32>,
        SolverResult<Assignment<i32>, SimpleScore>,
        String,
    >::new(1)
    .unwrap();
    let job = manager
        .submit(
            1,
            p,
            Default::default(),
            move |p, _, token| {
                let metrics = if p.variables().len() == 2 {
                    initial_metrics.clone()
                } else {
                    let next = monitor(p);
                    shared
                        .lock()
                        .unwrap()
                        .rebind_problem_monitor(next.clone(), p)
                        .unwrap();
                    next
                };
                let mut paused = false;
                let mut events = |event: SolverEvent<'_, Assignment<i32>, SimpleScore>| {
                    if let SolverEvent::Best(update) = event {
                        shared.lock().unwrap().publish_solver_meters().unwrap();
                        if p.variables().len() == 2 && update.complete && !paused {
                            paused = true;
                            started.send(()).unwrap();
                            released.recv_timeout(WAIT).unwrap();
                        }
                    }
                };
                solve_with_extensions(
                    p,
                    &ConstraintNetwork::<_, SimpleScore>::new(p.variables().len()),
                    configuration(Some(token)),
                    SolverExtensions {
                        metrics: Some(&metrics),
                        events: Some(&mut events),
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())
            },
            |_, _| {},
        )
        .unwrap();
    start.recv_timeout(WAIT).unwrap();
    let change = job
        .add_problem_change_with_previous(|p, old| {
            let old = old.expect("initial run returned");
            let old_values = old.solution.assignment.values();
            let mut variables = p
                .variables()
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    let var = Variable::new(vec![0, 1, 2]);
                    old_values[i].map_or(var.clone(), |v| var.with_initial(v))
                })
                .collect::<Vec<_>>();
            variables.push(Variable::new(vec![0, 1, 2]));
            *p = FiniteProblem::new(variables).unwrap();
            Ok(())
        })
        .unwrap();
    release.send(()).unwrap();
    assert!(job.wait_timeout(WAIT).unwrap().as_ref().is_ok());
    assert_eq!(change.wait_timeout(WAIT), Some(Ok(())));
    let view = view.lock().unwrap();
    for name in [
        "timefold.solver.problem.entities",
        "timefold.solver.problem.variables",
        "timefold.solver.problem.values",
    ] {
        assert_eq!(
            view.last_published_meter(&expected_id(name, SolverMeterType::Gauge, true)),
            Some(3.0)
        );
    }
    drop(view);
    manager.shutdown().unwrap();
}
