//! Original end-to-end retained-cache benchmark; no upstream benchmark code.
//! Run: retained_snapshot_benchmark retained|ordinary WARMUPS SAMPLES
//! Network construction is outside timing; director initialization is included.
#[path = "../tests/support/snapshot_score.rs"]
mod owned;
use owned::OwnedScore;
use std::{cell::Cell, error::Error, hint::black_box, time::Instant};
use timetuck::*;
#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    variables: usize,
    constants: usize,
    dense: bool,
    descending: bool,
    cap: u64,
}
fn run<S: AdditiveScore<Level = i64>>(
    mode: &str,
    warmups: usize,
    samples: usize,
    score_name: &str,
    make: impl Fn(i64) -> S + Copy,
) -> Result<(), Box<dyn Error>> {
    for case in [
        Case {
            name: "small",
            variables: 2,
            constants: 0,
            dense: false,
            descending: false,
            cap: 4,
        },
        Case {
            name: "sparse",
            variables: 10,
            constants: 512,
            dense: false,
            descending: false,
            cap: 1024,
        },
        Case {
            name: "dense",
            variables: 10,
            constants: 128,
            dense: true,
            descending: false,
            cap: 1024,
        },
        Case {
            name: "no-improvement",
            variables: 10,
            constants: 512,
            dense: false,
            descending: true,
            cap: 1024,
        },
        Case {
            name: "one-evaluation",
            variables: 10,
            constants: 512,
            dense: false,
            descending: false,
            cap: 1,
        },
        Case {
            name: "zero-evaluations",
            variables: 10,
            constants: 512,
            dense: false,
            descending: false,
            cap: 0,
        },
    ] {
        let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]); case.variables])?;
        let calls = Cell::new(0u128);
        let mut n = ConstraintNetwork::new(case.variables);
        for i in 0..case.variables {
            let calls = &calls;
            n.add_constraint(format!("v{i}"), vec![i], move |v| {
                calls.set(calls.get() + 1);
                make(
                    v[0].copied().unwrap_or(0)
                        * (1 << (case.variables - i - 1))
                        * if case.descending { -1 } else { 1 },
                )
            })?;
        }
        for i in 0..case.constants {
            let calls = &calls;
            let scope = if case.dense {
                (0..case.variables).collect()
            } else {
                vec![]
            };
            n.add_constraint(format!("extra{i}"), scope, move |v| {
                calls.set(calls.get() + 1);
                make(v.iter().filter_map(|v| v.copied()).sum())
            })?;
        }
        for sample in 0..warmups + samples {
            calls.set(0);
            owned::reset_clones();
            let config = SolverSequence {
                phases: vec![SolverPhase::Exact(Default::default())],
                limits: SolverLimits {
                    max_evaluations: case.cap,
                    ..Default::default()
                },
            };
            let start = Instant::now();
            let (r, setup_ns) = if mode == "ordinary" {
                (solve(black_box(&p), black_box(&n), config)?, 0)
            } else {
                let mut d =
                    ScoreDirector::new(black_box(&p), black_box(&n), p.initial_assignment())?;
                let setup_ns = start.elapsed().as_nanos();
                let r = d.run_solver(config)?;
                black_box(d);
                (r, setup_ns)
            };
            let total_ns = start.elapsed().as_nanos();
            black_box(&r);
            if sample >= warmups {
                println!(
                    "{}",
                    serde_json::json!({"mode":mode,"score":score_name,"case":case.name,"sample":sample-warmups,"total_ns":total_ns,"setup_ns":setup_ns,"score_clones":owned::clones(),"callbacks_including_setup":calls.get(),"logical_evaluations":r.work.evaluations,"reported_callbacks":r.work.constraint_evaluations,"score_levels":r.score.as_ref().map(|s|s.ordinary_levels().collect::<Vec<_>>()),"assignment":r.assignment.values(),"complete":r.complete})
                );
            }
        }
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [mode, warmups, samples] = args.as_slice() else {
        return Err("expected retained|ordinary warmups samples".into());
    };
    if !matches!(mode.as_str(), "retained" | "ordinary") {
        return Err("unknown mode".into());
    }
    let warmups: usize = warmups.parse()?;
    let samples: usize = samples.parse()?;
    if samples == 0 {
        return Err("samples must be positive".into());
    }
    run(mode, warmups, samples, "simple", SimpleScore::new)?;
    run(mode, warmups, samples, "owned", |s| OwnedScore::new(0, s))
}
