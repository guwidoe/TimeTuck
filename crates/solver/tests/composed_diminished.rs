//! Original composed-rate contracts. Existing DiminishedReturns supplies the arithmetic.
use std::{
    cell::Cell,
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
fn rate(window: u64) -> StatefulSearchTermination<'static> {
    StatefulSearchTermination::Condition(StatefulSearchLimit::DiminishedReturns(
        PhaseDiminishedReturnsConfig::new(
            DiminishedReturnsConfig::new(ms(window), 1, 2).unwrap(),
            &SimpleScore::ZERO,
        )
        .unwrap(),
    ))
}
fn progress(now: u64, steps: u64) -> SearchProgress {
    SearchProgress {
        elapsed: ms(now),
        completed_steps: steps,
        ..Default::default()
    }
}
#[test]
fn session_matches_existing_policy_and_reset_discards_grace_and_expiry() {
    let expression = rate(10);
    assert_eq!(
        expression.try_status(progress(1000, 0), Some(&SimpleScore::ZERO)),
        Err(StatefulTerminationError::HistoryRequired)
    );
    let mut session = StatefulTerminationSession::new(&expression);
    // Setup and static best arguments cannot initialize this history.
    session.observe_best(ms(1000), &SimpleScore::new(100));
    assert_eq!(
        session
            .status(progress(2000, 0), Some(&SimpleScore::new(100)))
            .unwrap(),
        TerminationState {
            reached: false,
            progress: None
        }
    );
    session.start_search(ms(2000));
    session.start_step(ms(2000));
    session.observe_best(ms(2000), &SimpleScore::ZERO);
    let mut direct =
        DiminishedReturns::<SimpleScore>::new(DiminishedReturnsConfig::new(ms(10), 1, 2).unwrap())
            .unwrap();
    direct.start(ms(2000), SimpleScore::ZERO);
    for (time, value) in [
        (2000, 0),
        (2005, 5),
        (2010, 10),
        (2015, 12),
        (2020, 12),
        (2030, 100),
    ] {
        let score = SimpleScore::new(value);
        session.observe_best(ms(time), &score);
        let state = session.status(progress(time, 0), Some(&score)).unwrap();
        let expected = direct.is_terminated(ms(time), &score);
        assert_eq!(state.reached, expected, "{time}");
        assert_eq!(state.progress, expected.then_some(1.0));
    }
    session.reset();
    assert!(
        !session
            .status(progress(50_000, 0), Some(&SimpleScore::new(100)))
            .unwrap()
            .reached
    );
    session.start_search(ms(50_000));
    session.start_step(ms(50_000));
    session.observe_best(ms(50_000), &SimpleScore::new(100));
    assert!(!session.status(progress(50_000, 0), None).unwrap().reached);
    assert!(!session.status(progress(10, 0), None).unwrap().reached); // backwards readings clamp
    assert!(session.status(progress(50_010, 0), None).unwrap().reached);
}
#[test]
fn every_independent_window_is_sampled_while_an_earlier_and_leaf_is_false() {
    let count =
        ScoredSearchTermination::Condition(ScoredSearchLimit::Search(SearchLimit::Steps(4)));
    let children = [
        StatefulSearchTermination::Condition(StatefulSearchLimit::Static(&count)),
        rate(10),
        rate(20),
    ];
    let expression = StatefulSearchTermination::All(&children);
    let mut s = StatefulTerminationSession::new(&expression);
    s.start_search(ms(0));
    s.start_step(ms(0));
    s.observe_best(ms(0), &SimpleScore::ZERO);
    assert!(!s.status(progress(0, 0), None).unwrap().reached);
    assert!(!s.status(progress(10, 4), None).unwrap().reached); // longer window not expired
    assert!(!s.status(progress(20, 3), None).unwrap().reached); // both windows expired
    assert!(s.status(progress(21, 4), None).unwrap().reached);
    let any = StatefulSearchTermination::Any(&children[1..]);
    let mut s = StatefulTerminationSession::new(&any);
    s.start_search(ms(0));
    s.start_step(ms(0));
    s.observe_best(ms(0), &SimpleScore::ZERO);
    assert!(!s.status(progress(0, 0), None).unwrap().reached);
    assert!(s.status(progress(10, 0), None).unwrap().reached);
}
#[test]
fn higher_level_progress_resets_grace_and_worsening_or_partial_scores_do_not() {
    let expression = StatefulSearchTermination::Condition(StatefulSearchLimit::DiminishedReturns(
        PhaseDiminishedReturnsConfig::new(
            DiminishedReturnsConfig::new(ms(10), 1, 2).unwrap(),
            &HardSoftScore::ZERO,
        )
        .unwrap(),
    ));
    let mut s = StatefulTerminationSession::new(&expression);
    s.start_search(ms(0));
    s.start_step(ms(0));
    s.observe_best(ms(0), &HardSoftScore::new(-1, 0));
    assert!(!s.status(progress(0, 0), None).unwrap().reached);
    s.observe_best(ms(8), &HardSoftScore::new(0, 2));
    s.observe_best(ms(12), &HardSoftScore::new(-1, 1000));
    s.observe_best(ms(14), &HardSoftScore::new(5, 500).with_structural(-1));
    assert!(!s.status(progress(17, 0), None).unwrap().reached);
    assert!(s.status(progress(18, 0), None).unwrap().reached);
}
fn local(steps: u64) -> SolverPhase<'static> {
    SolverPhase::LocalSearch(LocalSearchConfig {
        max_steps: steps,
        acceptance: Acceptance::HillClimbing,
        ..Default::default()
    })
}
#[test]
fn each_phase_rearms_after_setup_and_does_not_stop_the_next_phase() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let t = Arc::new(AtomicU64::new(0));
    let ticks = t.clone();
    let clock = SolverClock::new(move || ms(ticks.load(Ordering::SeqCst)));
    let condition = rate(5);
    let bindings = [
        PhaseTerminationBinding::stateful(0, &condition),
        PhaseTerminationBinding::stateful(1, &condition),
    ];
    let mut listener = |event| match event {
        SolverLifecycleEvent::PhaseStarted { .. } => {
            t.fetch_add(1000, Ordering::SeqCst);
        }
        SolverLifecycleEvent::StepStarted { .. } => {
            t.fetch_add(3, Ordering::SeqCst);
        }
        _ => {}
    };
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![local(100), local(100)],
            ..Default::default()
        },
        SolverExtensions {
            clock: Some(&clock),
            lifecycle: Some(&mut listener),
            phase_terminations: &bindings,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.phases.len(), 2);
    assert_eq!(r.termination, SolverTermination::PlanCompleted);
    for phase in &r.phases {
        assert_eq!(phase.work.completed_steps, 1);
        assert_eq!(phase.work.move_evaluations, 1);
        assert_eq!(
            phase.termination,
            PhaseTermination::LocalSearch(LocalSearchTermination::StepCondition)
        );
    }
    assert_eq!(r.assignment, p.initial_assignment());
}
#[test]
fn unsupported_scope_is_rejected_before_clock_lifecycle_scoring_or_factory() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let condition = rate(5);
    let clock = SolverClock::new(|| panic!("invalid binding sampled clock"));
    let factories = Cell::new(0);
    for phase in [
        SolverPhase::Construction(Default::default()),
        SolverPhase::Exact(Default::default()),
        SolverPhase::Custom(Default::default()),
        SolverPhase::RuinRecreate(Default::default()),
        local(0),
    ] {
        let bindings = [PhaseTerminationBinding::stateful(0, &condition)];
        let solve_wide = matches!(phase, SolverPhase::LocalSearch(_));
        let mut lifecycle = |_| panic!("invalid binding called lifecycle");
        let mut command = |_: usize,
                           _: &mut CustomPhaseContext<
            '_,
            Assignment<i64>,
            Move<i64>,
            SimpleScore,
            StreamScoringError,
        >|
         -> Result<(), CustomPhaseError<StreamScoringError>> {
            panic!("invalid binding invoked command")
        };
        let r = solve_stream_with_extensions(
            &p,
            || {
                factories.set(factories.get() + 1);
                panic!("invalid binding created graph");
            },
            SolverSequence {
                phases: vec![phase],
                limits: SolverLimits {
                    max_evaluations: 0,
                    ..Default::default()
                },
            },
            SolverExtensions {
                clock: Some(&clock),
                lifecycle: Some(&mut lifecycle),
                command: Some(&mut command),
                phase_terminations: if solve_wide { &[] } else { &bindings },
                stateful_termination: solve_wide.then_some(&condition),
                ..Default::default()
            },
        );
        assert!(
            matches!(r,Err(SolverError::DiminishedReturnsRequiresLocalSearch{phase}) if phase==if solve_wide {None}else{Some(0)})
        );
    }
    assert_eq!(factories.get(), 0);
    // Validation also runs in the direct scoped path.
    assert!(matches!(
        solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![local(0)],
                ..Default::default()
            },
            SolverExtensions {
                stateful_termination: Some(&condition),
                ..Default::default()
            }
        ),
        Err(SolverError::DiminishedReturnsRequiresLocalSearch { phase: None })
    ));
}
#[test]
fn expression_cooling_uses_supported_step_progress_alongside_the_rate_leaf() {
    let rate = rate(5);
    let steps =
        ScoredSearchTermination::Condition(ScoredSearchLimit::Search(SearchLimit::Steps(2)));
    let children = [
        rate,
        StatefulSearchTermination::Condition(StatefulSearchLimit::Static(&steps)),
    ];
    let condition = StatefulSearchTermination::All(&children);
    let mut state = StatefulTerminationSession::new(&condition);
    state.start_search(ms(0));
    state.start_step(ms(0));
    state.observe_best(ms(0), &SimpleScore::ZERO);
    assert_eq!(
        state.status(progress(0, 0), None).unwrap().progress,
        Some(0.0)
    );
    assert_eq!(
        state.status(progress(6, 1), None).unwrap().progress,
        Some(0.5)
    );
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::<i64, SimpleScore>::new(1);
    n.add_constraint("cost", vec![0], |v| {
        SimpleScore::new(-v[0].copied().unwrap())
    })
    .unwrap();
    let t = Arc::new(AtomicU64::new(0));
    let ticks = t.clone();
    let clock = SolverClock::new(move || ms(ticks.load(Ordering::SeqCst)));
    let mut listener = |event| {
        if matches!(event, SolverLifecycleEvent::StepStarted { .. }) {
            t.fetch_add(3, Ordering::SeqCst);
        }
    };
    let mut provider = |_: &FiniteProblem<i64>, a: &Assignment<i64>, _: u64| {
        vec![Move::Change {
            variable: 0,
            value: Some(1 - a.values()[0].unwrap()),
        }]
    };
    let bindings = [PhaseTerminationBinding::stateful(0, &condition)];
    let mut ext = SolverExtensions {
        clock: Some(&clock),
        lifecycle: Some(&mut listener),
        phase_terminations: &bindings,
        ..Default::default()
    };
    ext.move_providers.bind(0, &mut provider);
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 10,
                acceptance: Acceptance::SimulatedAnnealingTermination {
                    starting_temperature: SimpleScore::new(1_000_000),
                    scope: TerminationProgressScope::Phase,
                    seed: 11,
                },
                ..Default::default()
            })],
            ..Default::default()
        },
        ext,
    )
    .unwrap();
    assert_eq!(r.work.completed_steps, 2);
    assert_eq!(r.work.move_evaluations, 2);
    assert_eq!(r.assignment, p.initial_assignment());
    assert_eq!(
        r.phases[0].termination,
        PhaseTermination::LocalSearch(LocalSearchTermination::StepCondition)
    );
}
#[test]
fn empty_score_shape_fails_at_configuration_without_a_runtime_baseline() {
    assert_eq!(
        PhaseDiminishedReturnsConfig::new(
            DiminishedReturnsConfig::new(ms(10), 1, 2).unwrap(),
            &BendableScore::<0, 0>::ZERO
        )
        .unwrap_err(),
        DiminishedReturnsError::NoScoreLevels
    );
}
#[test]
fn no_complete_search_best_means_no_grace_and_score_only_types_are_supported() {
    #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
    struct Owned(i64, String);
    impl Score for Owned {
        type Level = i64;
        const HARD_LEVEL_COUNT: usize = 0;
        fn ordinary_levels(&self) -> impl Iterator<Item = i64> {
            std::iter::once(self.0)
        }
        fn structural(&self) -> i64 {
            0
        }
        fn hard_cmp(&self, _: &Self) -> std::cmp::Ordering {
            std::cmp::Ordering::Equal
        }
        fn is_feasible(&self) -> bool {
            true
        }
    }
    let prototype = Owned(999, "prototype".into());
    let condition = StatefulSearchTermination::Condition(StatefulSearchLimit::DiminishedReturns(
        PhaseDiminishedReturnsConfig::new(
            DiminishedReturnsConfig::new(ms(10), 1, 2).unwrap(),
            &prototype,
        )
        .unwrap(),
    ));
    let mut session = StatefulTerminationSession::new(&condition);
    session.start_search(ms(0));
    session.start_step(ms(0));
    assert_eq!(
        session.status(progress(100, 0), Some(&prototype)).unwrap(),
        TerminationState {
            reached: false,
            progress: None
        }
    );
    session.observe_best(ms(100), &Owned(0, "best".into()));
    assert!(!session.status(progress(100, 0), None).unwrap().reached);
    assert!(session.status(progress(110, 0), None).unwrap().reached);
    let condition = rate(10);
    let mut session = StatefulTerminationSession::new(&condition);
    session.start_search(ms(0));
    session.start_step(ms(0));
    session.observe_best(ms(0), &SimpleScore::new(100).with_structural(-1));
    assert!(!session.status(progress(100, 0), None).unwrap().reached);
    session.observe_best(ms(100), &SimpleScore::ZERO);
    assert!(!session.status(progress(100, 0), None).unwrap().reached);
    assert!(session.status(progress(110, 0), None).unwrap().reached);
}
