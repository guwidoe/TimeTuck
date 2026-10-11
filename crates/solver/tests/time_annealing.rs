//! Original duration-cooling contracts; no upstream method credit.
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use timetuck::*;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn timed(temperature: i64, duration: u64) -> Acceptance {
    Acceptance::SimulatedAnnealingTime {
        starting_temperature: SimpleScore::new(temperature),
        cooling_time: ms(duration),
        seed: 42,
    }
}
#[test]
fn duration_gradient_validation_freezing_and_seeded_reference() {
    assert!(matches!(
        TimeSimulatedAnnealing::new(SimpleScore::ZERO, Duration::ZERO, 0),
        Err(AcceptanceError::ZeroCoolingTime)
    ));
    assert!(matches!(
        TimeSimulatedAnnealing::new(SimpleScore::new(-1), ms(1), 0),
        Err(AcceptanceError::InvalidTemperature)
    ));
    let mut time = TimeSimulatedAnnealing::new(SimpleScore::new(200), ms(100), 42).unwrap();
    let mut steps = SimulatedAnnealing::new(SimpleScore::new(200), 2, 42).unwrap();
    for elapsed in [0, 50, 100] {
        time.step_started(ms(elapsed));
        assert_eq!(time.gradient(), steps.gradient());
        for candidate in (-1400..=-400).step_by(11) {
            let current = SimpleScore::new(-1000);
            let candidate = SimpleScore::new(candidate);
            assert_eq!(
                time.probability(current, candidate),
                steps
                    .probability_at(current, candidate, steps.gradient())
                    .unwrap()
            );
            assert_eq!(
                time.accepts(current, candidate),
                steps.accepts(current, candidate)
            );
        }
        assert_eq!(time.gradient(), elapsed as f64 / 100.0);
        steps.step_ended();
    }
    time.step_started(Duration::ZERO);
    assert_eq!(time.gradient(), 1.0);
    let mut huge = TimeSimulatedAnnealing::new(SimpleScore::new(100), Duration::MAX, 42).unwrap();
    huge.step_started(Duration::MAX / 2);
    assert!((huge.gradient() - 0.5).abs() < 1e-15);
    let mut cloned = huge.clone();
    for _ in 0..100 {
        assert_eq!(
            huge.accepts(SimpleScore::ZERO, SimpleScore::new(-10)),
            cloned.accepts(SimpleScore::ZERO, SimpleScore::new(-10))
        );
    }
    huge.step_started(Duration::MAX);
    assert_eq!(huge.gradient(), 1.0);
    assert!(cloned.gradient() < 1.0);
}
#[test]
fn composite_short_circuit_keeps_random_stream_and_samples_every_child() {
    let policies = [
        Acceptance::MoveTabu {
            tenure: 1,
            aspiration: false,
        },
        timed(100, 100),
    ];
    let mut combined = MoveAcceptor::from_policies(&policies, SimpleScore::ZERO).unwrap();
    let mut reference = timed(100, 100).start(SimpleScore::ZERO).unwrap();
    combined.step_ended(SimpleScore::ZERO, &[0], &0);
    combined.step_started(ms(50));
    reference.step_started(ms(50));
    for _ in 0..20 {
        assert!(!combined.accepts(SimpleScore::ZERO, SimpleScore::new(-10), &[0], &0));
    }
    for _ in 0..100 {
        assert_eq!(
            combined.accepts(SimpleScore::ZERO, SimpleScore::new(-10), &[1], &1),
            reference.accepts(SimpleScore::ZERO, SimpleScore::new(-10), &[1])
        );
    }
    combined.step_started(ms(100));
    assert!(!combined.accepts(SimpleScore::ZERO, SimpleScore::new(-1), &[1], &1));
    let mut values =
        ValueAcceptor::<_, i32, i32>::from_policies(&[timed(100, 100)], SimpleScore::ZERO).unwrap();
    values.step_started(ms(100));
    assert!(!values.accepts(SimpleScore::ZERO, SimpleScore::new(-1), &[0], &0, &[0]));
}
#[test]
fn candidate_scan_freezes_temperature_then_next_step_observes_elapsed() {
    let ticks = Arc::new(AtomicU64::new(100));
    let source = ticks.clone();
    let reads = Arc::new(AtomicU64::new(0));
    let counter = reads.clone();
    let clock = SolverClock::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        ms(source.load(Ordering::SeqCst))
    });
    let p = FiniteProblem::new(vec![Variable::new(vec![0, -1, -2, -3]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::<i64, SimpleScore>::new(1);
    n.add_constraint("value", vec![0], |v| SimpleScore::new(*v[0].unwrap()))
        .unwrap();
    let seen = RefCell::new(Vec::new());
    let sampled = RefCell::new(Vec::new());
    let mut provider = |_: &FiniteProblem<i64>, a: &Assignment<i64>, step: u64| {
        seen.borrow_mut().push(a.values()[0].unwrap());
        sampled.borrow_mut().push(reads.load(Ordering::SeqCst));
        ticks.store(110, Ordering::SeqCst);
        [-2, if step == 0 { -1 } else { -3 }]
            .map(|v| Move::Change {
                variable: 0,
                value: Some(v),
            })
            .to_vec()
    };
    let additional = [timed(1_000_000_000, 5)];
    let mut ext = SolverExtensions {
        clock: Some(&clock),
        ..Default::default()
    };
    ext.move_providers.bind(0, &mut provider);
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 5,
                selection: MoveSelection::BestAccepted,
                acceptance: Acceptance::MoveTabu {
                    tenure: 1,
                    aspiration: true,
                },
                additional_acceptance: &additional,
                ..Default::default()
            })],
            ..Default::default()
        },
        ext,
    )
    .unwrap();
    assert_eq!(*seen.borrow(), vec![0, -1]);
    assert_eq!(r.work.completed_steps, 1);
    assert_eq!(
        r.work.accepted_moves, 1,
        "the first scan selects its later hot candidate"
    );
    assert_eq!(r.work.move_evaluations, 4);
    let samples = sampled.borrow();
    assert_eq!(samples[1], samples[0] + 1, "one clock sample between scans");
    assert_eq!(
        reads.load(Ordering::SeqCst),
        samples[1],
        "no candidate or teardown temperature samples"
    );
    assert_eq!(r.assignment, p.initial_assignment());
    assert_eq!(r.score, Some(SimpleScore::ZERO));
}
#[test]
fn unused_duration_policy_does_not_sample_custom_clock() {
    let reads = Arc::new(AtomicU64::new(0));
    let count = reads.clone();
    let clock = SolverClock::new(move || {
        count.fetch_add(1, Ordering::SeqCst);
        Duration::ZERO
    });
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::<i64, SimpleScore>::new(1);
    n.add_constraint("value", vec![0], |v| SimpleScore::new(*v[0].unwrap()))
        .unwrap();
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 3,
                acceptance: Acceptance::SimulatedAnnealing {
                    starting_temperature: SimpleScore::new(100),
                    cooling_steps: 3,
                    seed: 42,
                },
                ..Default::default()
            })],
            ..Default::default()
        },
        SolverExtensions {
            clock: Some(&clock),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.work.completed_steps, 3);
    assert_eq!(
        reads.load(Ordering::SeqCst),
        1,
        "only solve-entry rebasing reads the source"
    );
}
#[test]
fn zero_duration_rejected_before_lifecycle_or_factory() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let mut callback = |_| panic!("invalid policy must not invoke lifecycle");
    let r = solve_calculator_with_extensions(
        &p,
        &|| -> NoCalculator { panic!("invalid policy must not create calculator") },
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                acceptance: timed(100, 0),
                ..Default::default()
            })],
            ..Default::default()
        },
        SolverExtensions {
            lifecycle: Some(&mut callback),
            ..Default::default()
        },
    );
    assert!(matches!(
        r,
        Err(SolverError::InvalidAcceptance {
            phase: 0,
            error: AcceptanceError::ZeroCoolingTime
        })
    ));
}
struct NoCalculator;
impl IncrementalCalculator<Assignment<i32>> for NoCalculator {
    type Score = SimpleScore;
    type Justification = ();
    type Change = Vec<usize>;
    fn reset(
        &mut self,
        _: &Assignment<i32>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        unreachable!()
    }
    fn calculate_score(&mut self) -> SimpleScore {
        unreachable!()
    }
}

#[test]
fn repair_attempts_sample_phase_time_and_reset_without_changing_best() {
    let ticks = Arc::new(AtomicU64::new(100));
    let source = ticks.clone();
    let clock = SolverClock::new(move || ms(source.load(Ordering::SeqCst)));
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::<i64, SimpleScore>::new(1);
    n.add_constraint("value", vec![0], |v| SimpleScore::new(-*v[0].unwrap()))
        .unwrap();
    let config = RuinRecreateConfig {
        max_attempts: 2,
        acceptance: timed(1_000_000_000, 5),
        move_weights: RepairMoveWeights {
            change: 1,
            swap: 0,
            repair: 0,
        },
        ..Default::default()
    };
    let plan = SolverSequence {
        phases: vec![SolverPhase::RuinRecreate(config); 2],
        ..Default::default()
    };
    for retained in [false, true] {
        let mut callback = |event| {
            if matches!(event, SolverLifecycleEvent::PhaseStarted { phase: 0, .. }) {
                ticks.fetch_add(10, Ordering::SeqCst);
            }
        };
        let ext = SolverExtensions {
            clock: Some(&clock),
            lifecycle: Some(&mut callback),
            ..Default::default()
        };
        let r = if retained {
            let mut director = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
            let r = director
                .run_solver_with_extensions(plan.clone(), ext)
                .unwrap();
            assert_eq!(director.assignment(), &r.assignment);
            director.assert_consistent().unwrap();
            r
        } else {
            solve_with_extensions(&p, &n, plan.clone(), ext).unwrap()
        };
        assert_eq!(r.phases[0].work.accepted_moves, 0);
        assert_eq!(r.phases[1].work.accepted_moves, 2);
        assert_eq!(r.assignment, p.initial_assignment());
        assert_eq!(r.score, Some(SimpleScore::ZERO));
    }
}

#[test]
fn factory_preparation_is_charged_without_lifecycle_or_termination_clock() {
    let ticks = Arc::new(AtomicU64::new(100));
    let source = ticks.clone();
    let clock = SolverClock::new(move || ms(source.load(Ordering::SeqCst)));
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let r = solve_stream_with_extensions(
        &p,
        || {
            ticks.fetch_add(10, Ordering::SeqCst);
            let mut g = StreamGraphBuilder::new();
            let s = g.source::<usize, (usize, Option<i64>)>();
            g.score("value", s.port(), |(_, v)| {
                SimpleScore::new(-v.unwrap_or(0))
            })
            .unwrap();
            Ok((g.build(), s))
        },
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 2,
                acceptance: timed(1_000_000_000, 5),
                ..Default::default()
            })],
            ..Default::default()
        },
        SolverExtensions {
            clock: Some(&clock),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.work.completed_steps, 0);
    assert!(r.work.move_evaluations > 0);
    assert_eq!(r.assignment, p.initial_assignment());
}
