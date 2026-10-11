//! Original local/custom end-to-end benchmark, with ordinary-solve countercases.
//! Usage: retained_local_snapshot_benchmark retained|ordinary local|custom WARMUP SAMPLES
//! Problem/network setup is excluded. Retained director setup and all scoring,
//! candidate selection, snapshot cloning, restoration and destruction are included.
#[path = "../tests/support/snapshot_score.rs"]
mod owned;
use owned::OwnedScore;
use std::{cell::Cell, error::Error, hint::black_box, time::Instant};
use timetuck::*;
#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    extra: usize,
    dense: bool,
    flat: bool,
    steps: u64,
    cap: u64,
}
fn change(variable: usize, value: i64) -> Move<i64> {
    Move::Change {
        variable,
        value: Some(value),
    }
}
fn run<S: AdditiveScore<Level = i64>>(
    mode: &str,
    phase: &str,
    warmups: usize,
    samples: usize,
    score_name: &str,
    make: impl Fn(i64) -> S + Copy,
) -> Result<(), Box<dyn Error>> {
    for case in [
        Case {
            name: "small",
            extra: 0,
            dense: false,
            flat: false,
            steps: 4,
            cap: 10000,
        },
        Case {
            name: "sparse",
            extra: 512,
            dense: false,
            flat: false,
            steps: 128,
            cap: 10000,
        },
        Case {
            name: "dense",
            extra: 128,
            dense: true,
            flat: false,
            steps: 128,
            cap: 10000,
        },
        Case {
            name: "no-improvement",
            extra: 512,
            dense: false,
            flat: true,
            steps: 128,
            cap: 10000,
        },
        Case {
            name: "initial-only",
            extra: 512,
            dense: false,
            flat: false,
            steps: 0,
            cap: 10000,
        },
        Case {
            name: "zero-evaluations",
            extra: 512,
            dense: false,
            flat: false,
            steps: 128,
            cap: 0,
        },
    ] {
        let p = FiniteProblem::new(vec![
            Variable::new((-1..=case.steps as i64).collect())
                .with_initial(0);
            2
        ])?;
        let calls = Cell::new(0u128);
        let mut n = ConstraintNetwork::new(2);
        for i in 0..2 {
            let calls = &calls;
            n.add_constraint(format!("v{i}"), vec![i], move |v| {
                calls.set(calls.get() + 1);
                make(if case.flat {
                    0
                } else {
                    v[0].copied().unwrap_or(0)
                })
            })?;
        }
        for i in 0..case.extra {
            let calls = &calls;
            let scope = if case.dense { vec![0, 1] } else { vec![] };
            n.add_constraint(format!("extra{i}"), scope, move |v| {
                calls.set(calls.get() + 1);
                make(if case.flat {
                    0
                } else {
                    v.iter().filter_map(|v| v.copied()).sum()
                })
            })?;
        }
        for sample in 0..warmups + samples {
            let mut provider = |_: &FiniteProblem<i64>, a: &Assignment<i64>, _: u64| {
                vec![change(0, a.values()[0].unwrap() + 1), change(1, -1)]
            };
            let mut command =
                |_: usize,
                 c: &mut CustomPhaseContext<'_, Assignment<i64>, Move<i64>, S, ScoringError>|
                 -> Result<(), CustomPhaseError<ScoringError>> {
                    for v in 1..=case.steps as i64 {
                        c.evaluate(&change(1, -1), |_, _| ())?;
                        if c.execute(&change(0, v))?.is_none() {
                            break;
                        }
                    }
                    Ok(())
                };
            let mut providers = PhaseMoveProviders::default();
            let phases = if phase == "local" {
                providers.bind(0, &mut provider);
                vec![SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: case.steps,
                    selection: MoveSelection::BestAccepted,
                    acceptance: Acceptance::HillClimbing,
                    ..Default::default()
                })]
            } else {
                vec![SolverPhase::Custom(Default::default())]
            };
            let config = SolverSequence {
                phases,
                limits: SolverLimits {
                    max_evaluations: case.cap,
                    ..Default::default()
                },
            };
            let extensions = SolverExtensions {
                move_providers: providers,
                command: if phase == "custom" {
                    Some(&mut command)
                } else {
                    None
                },
                ..Default::default()
            };
            calls.set(0);
            owned::reset_clones();
            let start = Instant::now();
            let (r, setup_ns) = if mode == "ordinary" {
                (
                    solve_with_extensions(black_box(&p), black_box(&n), config, extensions)?,
                    0,
                )
            } else {
                let mut d =
                    ScoreDirector::new(black_box(&p), black_box(&n), p.initial_assignment())?;
                let setup = start.elapsed().as_nanos();
                let r = d.run_solver_with_extensions(config, extensions)?;
                black_box(d);
                (r, setup)
            };
            let elapsed = start.elapsed().as_nanos();
            black_box(&r);
            if sample >= warmups {
                println!(
                    "{}",
                    serde_json::json!({"mode":mode,"phase":phase,"score":score_name,"case":case.name,"sample":sample-warmups,"total_ns":elapsed,"setup_ns":setup_ns,"score_clones":owned::clones(),"callbacks_including_setup":calls.get(),"logical_evaluations":r.work.evaluations,"reported_callbacks":r.work.constraint_evaluations,"score_levels":r.score.as_ref().map(|s|s.ordinary_levels().collect::<Vec<_>>()),"assignment":r.assignment.values(),"complete":r.complete,"termination":format!("{:?}",r.termination)})
                );
            }
        }
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [mode, phase, warmups, samples] = args.as_slice() else {
        return Err("expected retained|ordinary local|custom warmups samples".into());
    };
    if !matches!(mode.as_str(), "retained" | "ordinary")
        || !matches!(phase.as_str(), "local" | "custom")
    {
        return Err("unknown mode or phase".into());
    }
    let warmups: usize = warmups.parse()?;
    let samples: usize = samples.parse()?;
    if samples == 0 {
        return Err("samples must be positive".into());
    }
    run(mode, phase, warmups, samples, "simple", SimpleScore::new)?;
    run(mode, phase, warmups, samples, "owned", |s| {
        OwnedScore::new(0, s)
    })
}
