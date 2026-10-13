//! Original per-session reseeding, replay and configuration checks.
#[path = "fixtures/assembled_factory_fixture.rs"]
mod fixture;
use fixture::*;
use std::{num::NonZeroUsize, rc::Rc};
use timetuck::*;
fn phases() -> Vec<SolverPhase<'static>> {
    vec![
        SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: 1,
            max_evaluations: u64::MAX,
            move_order: MoveOrder::ShuffledMoves {
                seed: 99,
                window: NonZeroUsize::new(8).unwrap()
            },
            tie_breaking: TieBreaking::Random { seed: 98 },
            acceptance: Acceptance::FadingMoveTabu {
                hard_tenure: 1,
                fading_tenure: 1,
                aspiration: true,
                seed: 789
            },
            ..Default::default()
        });
        2
    ]
}
fn factory(d: &Domain, mode: EnvironmentMode) -> Factory<'_> {
    let mut c = configuration(d, Some(mode), vec![None, None], 1);
    c.seed = 123;
    c.phases = phases();
    declaration(d, false, Rc::new(Observations::default()))
        .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
        .unwrap()
}
fn inspect(phase: &SolverPhase<'_, SimpleScore>, seed: u64, index: u64, clamped: bool) {
    let SolverPhase::LocalSearch(c) = phase else {
        panic!()
    };
    assert_eq!(
        c.move_order,
        MoveOrder::ShuffledMoves {
            seed: seed.wrapping_add(index),
            window: NonZeroUsize::new(8).unwrap()
        }
    );
    assert_eq!(
        c.tie_breaking,
        TieBreaking::Random {
            seed: seed.wrapping_add(index)
        }
    );
    assert!(matches!(
        c.acceptance,
        Acceptance::FadingMoveTabu { seed: 789, .. }
    ));
    if clamped {
        assert!(c.max_evaluations <= 100);
    } else {
        assert_eq!(c.max_evaluations, u64::MAX);
    }
}
#[test]
fn concurrent_sessions_and_standalone_openings_have_independent_fresh_seeds() {
    let d = Domain::new(3, 3, Objective::Weighted);
    let f = factory(&d, EnvironmentMode::NonReproducible);
    let mut a = f.start_assembled_run(d.model.initial_assignment()).unwrap();
    let mut b = f.start_assembled_run(d.model.initial_assignment()).unwrap();
    let sa = a.assembled_run_seed();
    let sb = b.assembled_run_seed();
    assert_ne!(sa, sb);
    let standalone = f.open_assembled_random().factory_stream_seed();
    assert_ne!(sa, standalone);
    assert_ne!(sb, standalone);
    let mut first = FactoryRandomStream::seeded_factory_stream(sa);
    let mut second = FactoryRandomStream::seeded_factory_stream(sb);
    for _ in 0..1000 {
        assert_ne!(first.next_factory_word(), second.next_factory_word());
    }
    for index in 0..2 {
        a.run_next_assembled_phase(|r, p| {
            inspect(p, sa, index, false);
            r.phase(p)
        })
        .unwrap();
        f.open_assembled_random();
        b.run_next_assembled_phase(|r, p| {
            inspect(p, sb, index, false);
            r.phase(p)
        })
        .unwrap();
        assert_eq!(a.assembled_run_seed(), sa);
        assert_eq!(b.assembled_run_seed(), sb);
    }
    a.finish_assembled_state().unwrap();
    b.finish_assembled_state().unwrap();
}
#[test]
fn reproducible_modes_keep_exact_seeded_streams_and_per_phase_offsets() {
    use EnvironmentMode::*;
    for mode in [
        NoAssert,
        PhaseAssert,
        StepAssert,
        FullAssert,
        NonIntrusiveFullAssert,
        TrackedFullAssert,
    ] {
        let d = Domain::new(3, 3, Objective::Weighted);
        let f = factory(&d, mode);
        for _ in 0..2 {
            let mut stream = f.open_assembled_random();
            let mut expected = FactoryRandomStream::seeded_factory_stream(123);
            for _ in 0..1000 {
                assert_eq!(stream.next_factory_word(), expected.next_factory_word());
            }
            let mut run = f.start_assembled_run(d.model.initial_assignment()).unwrap();
            assert_eq!(run.assembled_run_seed(), 123);
            for index in 0..2 {
                run.run_next_assembled_phase(|r, p| {
                    inspect(p, 123, index, false);
                    r.phase(p)
                })
                .unwrap();
            }
            run.finish_assembled_state().unwrap();
        }
    }
}
#[test]
fn observed_runs_reseed_before_budget_clamping_and_leave_template_unchanged() {
    let d = Domain::new(3, 3, Objective::Weighted);
    let f = factory(&d, EnvironmentMode::NonReproducible);
    let mut seeds = vec![];
    for _ in 0..3 {
        let mut callbacks = FactoryRunCallbacks::default();
        let mut run = f
            .start_observed_assembled_run_with_limits(
                d.model.initial_assignment(),
                FactoryRunLimits {
                    max_evaluations: Some(100),
                    max_time: None,
                },
                &mut callbacks,
            )
            .unwrap();
        let seed = run.assembled_run_seed();
        seeds.push(seed);
        for index in 0..2 {
            run.run_next_observed_phase(&mut callbacks, |r, p| {
                inspect(p, seed, index, true);
                r.phase(p)
            })
            .unwrap();
        }
        run.finish_observed_assembled_state(&mut callbacks).unwrap();
    }
    seeds.sort();
    seeds.dedup();
    assert_eq!(seeds.len(), 3);
    // A reproducible factory built from the same caller-supplied template retains
    // configured seed123; neither nonreproducible run changes caller data.
    let stable = factory(&d, EnvironmentMode::NoAssert);
    assert_eq!(stable.open_assembled_random().factory_stream_seed(), 123);
}
#[test]
fn ruin_recreation_receives_the_actual_session_seed() {
    let d = Domain::new(3, 3, Objective::Weighted);
    let mut c = configuration(&d, Some(EnvironmentMode::NonReproducible), vec![None], 1);
    c.phases = vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
        seed: 66,
        max_attempts: 1,
        minimum_ruined: 1,
        maximum_ruined: 1,
        ..Default::default()
    })];
    let f = declaration(&d, false, Rc::new(Observations::default()))
        .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
        .unwrap();
    let mut run = f.start_assembled_run(d.model.initial_assignment()).unwrap();
    let seed = run.assembled_run_seed();
    run.run_next_assembled_phase(|r, p| {
        let SolverPhase::RuinRecreate(c) = p else {
            panic!()
        };
        assert_eq!(c.seed, seed);
        r.phase(p)
    })
    .unwrap();
    run.finish_assembled_state().unwrap();
}
