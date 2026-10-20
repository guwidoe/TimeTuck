//! Original mixed solve benchmark; parsing/independent validation outside timing.
mod mixed_benchmark_support;
use mixed_benchmark_support::{Case, StreamGraphPolicy, run, run_cold, run_component_refinement};
use std::{
    error::Error,
    io::{self, BufRead},
    time::Instant,
};
fn main() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<_> = std::env::args().collect();
    const USAGE: &str = "mixed_solver_benchmark iterations warmup samples dense|stream [cold] [component-refinement] [--stream-graph-policy ordinary-join|owner-links|owner-links-fused]";
    if args.iter().skip(1).any(|a| a == "--help" || a == "-h") {
        println!(
            "{USAGE}\nDefault ordinary-join. Owner-links and owner-links-fused explicitly change match IDs, batching, ownership and physical score callbacks; fused v2 projects/scores each link directly; Dense is unaffected."
        );
        return Ok(());
    }
    let mut graph_policy = StreamGraphPolicy::OrdinaryJoin;
    if let Some(index) = args.iter().position(|a| a == "--stream-graph-policy") {
        graph_policy = match args.get(index + 1).map(String::as_str) {
            Some("ordinary-join") => StreamGraphPolicy::OrdinaryJoin,
            Some("owner-links") => StreamGraphPolicy::OwnerLinks,
            Some("owner-links-fused") => StreamGraphPolicy::FusedOwnerLinks,
            _ => return Err(
                "--stream-graph-policy requires ordinary-join, owner-links or owner-links-fused"
                    .into(),
            ),
        };
        args.drain(index..index + 2);
    }
    let (cold, component_refinement) = match args.get(5..).unwrap_or_default() {
        [] => (false, false),
        [mode] if mode == "cold" => (true, false),
        [profile] if profile == "component-refinement" => (false, true),
        [mode, profile] if mode == "cold" && profile == "component-refinement" => (true, true),
        _ => {
            return Err(
                "iterations warmup samples dense|stream [cold] [component-refinement]".into(),
            );
        }
    };
    if args.len() < 5 {
        return Err("iterations warmup samples dense|stream [cold] [component-refinement]".into());
    }
    let iterations: usize = args[1].parse()?;
    let warmup: usize = args[2].parse()?;
    let samples: usize = args[3].parse()?;
    let stream = match args[4].as_str() {
        "dense" => false,
        "stream" => true,
        _ => return Err("backend".into()),
    };
    if iterations == 0 || samples == 0 || iterations > 1000 || warmup > 100 || samples > 1000 {
        return Err("run bounds".into());
    }
    let line = io::stdin().lock().lines().next().ok_or("missing input")??;
    let mut case = Case::parse(&line)?;
    case.stream_graph_policy = graph_policy;
    if cold != case.quality_floor.is_some() {
        return Err("cold mode requires a quality floor; warm mode uses its initial score".into());
    }
    for sample in 0..warmup + samples {
        let (mut elapsed, mut hard, mut soft, mut evaluations, mut calls, mut steps) =
            (0u128, 0i64, 0i64, 0u64, 0u128, 0u64);
        let mut last = None;
        let (mut construction_steps, mut phase_evaluations) = (0u64, [0u64; 2]);
        let mut profile_phases: Option<Vec<(u64, u64, u64, String)>> = None;
        for _ in 0..iterations {
            let started = Instant::now();
            let (best, work, termination) = if component_refinement {
                let result = run_component_refinement(&case, stream, cold)?;
                elapsed += started.elapsed().as_nanos();
                if !result.complete
                    || result.work.completed_steps > case.steps
                    || result.work.evaluations > case.evaluations
                {
                    return Err("profile completeness or shared budget".into());
                }
                let local: Vec<_> = result
                    .phases
                    .iter()
                    .filter(|phase| {
                        matches!(
                            phase.termination,
                            timetuck::PhaseTermination::LocalSearch(_)
                        )
                    })
                    .collect();
                if local.len() > 1
                    && local[0].termination
                        != timetuck::PhaseTermination::LocalSearch(
                            timetuck::LocalSearchTermination::NoAcceptedMove,
                        )
                {
                    return Err("joint phase entered before component local optimum".into());
                }
                let phases: Vec<_> = result
                    .phases
                    .iter()
                    .map(|p| {
                        (
                            p.work.evaluations,
                            p.work.completed_steps,
                            p.work.construction_steps,
                            format!("{:?}", p.termination),
                        )
                    })
                    .collect();
                if let Some(sums) = &mut profile_phases {
                    if sums.len() != phases.len() {
                        return Err("nonrepeatable phase count".into());
                    }
                    for (sum, phase) in sums.iter_mut().zip(phases) {
                        if sum.3 != phase.3 {
                            return Err("nonrepeatable phase termination".into());
                        }
                        sum.0 += phase.0;
                        sum.1 += phase.1;
                        sum.2 += phase.2;
                    }
                } else {
                    profile_phases = Some(phases);
                }
                construction_steps += result.work.construction_steps;
                let best = timetuck::ScoredMixedAssignment {
                    assignment: result.assignment,
                    score: result.score.ok_or("missing score")?,
                };
                (best, result.work, format!("{:?}", result.termination))
            } else if cold {
                let result = run_cold(&case, stream)?;
                elapsed += started.elapsed().as_nanos();
                if !result.complete || result.phases.len() != 2 {
                    return Err("cold solve did not complete both phases".into());
                }
                construction_steps += result.work.construction_steps;
                for (sum, phase) in phase_evaluations.iter_mut().zip(&result.phases) {
                    *sum += phase.work.evaluations;
                }
                let best = timetuck::ScoredMixedAssignment {
                    assignment: result.assignment,
                    score: result.score.ok_or("missing score")?,
                };
                (best, result.work, format!("{:?}", result.termination))
            } else {
                let result = run(&case, stream)?;
                elapsed += started.elapsed().as_nanos();
                (
                    result.best.ok_or("missing best")?,
                    timetuck::SolverWork {
                        evaluations: result.evaluations,
                        constraint_evaluations: result.constraint_evaluations,
                        completed_steps: result.completed_steps,
                        ..Default::default()
                    },
                    format!("{:?}", result.termination),
                )
            };
            case.validate(&best.assignment, best.score)?;
            hard += best.score.hard();
            soft += best.score.soft();
            evaluations += work.evaluations;
            calls += work.constraint_evaluations;
            steps += work.completed_steps;
            last = Some((best, termination));
        }
        if sample >= warmup {
            let (best, termination) = last.unwrap();
            let mut row = serde_json::json!({"id":case.id,"elapsed_ns":u64::try_from(elapsed)?,"score":[hard,soft],"depots":best.assignment.basic().values(),"routes":best.assignment.lists().lists(),"evaluations":evaluations,"score_calls":u64::try_from(calls)?,"steps":steps,"termination":termination});
            row["requested_stream_graph_policy"] = graph_policy.name().into();
            row["stream_graph_policy"] = if stream {
                graph_policy.name()
            } else {
                "not-applicable-dense"
            }
            .into();
            if stream {
                let owner_links = graph_policy != StreamGraphPolicy::OrdinaryJoin;
                let fused = graph_policy == StreamGraphPolicy::FusedOwnerLinks;
                row["match_id_schema"] = if owner_links {
                    "((relationship_owner,left_owner),right_owner)"
                } else {
                    "(left_owner,right_owner)"
                }
                .into();
                row["publication_boundary"] = if owner_links {
                    "final-per-affected-link-per-director-batch"
                } else {
                    "ordinary-source-event-propagation"
                }
                .into();
                row["endpoint_ownership"] = if fused {
                    "shared-producer-events; fused full-pair projection-scoring"
                } else if owner_links {
                    "shared-producer-events; full-pair projection"
                } else {
                    "ordinary-join-owned-inputs"
                }
                .into();
                row["tracked_terminal"] = if fused {
                    "fused-fixed-links-full-payload-and-score"
                } else if owner_links {
                    "fixed-universe-full-payload-and-score"
                } else {
                    "ordinary-full-payload-and-score"
                }
                .into();
                row["physical_score_callback_metric"] = "score_calls includes actual route and any other score callbacks; policy counts may differ".into();
            }
            if component_refinement {
                let phases = profile_phases.unwrap_or_default();
                row["search_profile"] = "component-refinement".into();
                row["phase_evaluations"] = phases.iter().map(|p| p.0).collect::<Vec<_>>().into();
                row["phase_steps"] = phases.iter().map(|p| p.1).collect::<Vec<_>>().into();
                row["phase_construction_steps"] =
                    phases.iter().map(|p| p.2).collect::<Vec<_>>().into();
                row["phase_terminations"] = phases
                    .iter()
                    .map(|p| p.3.clone())
                    .collect::<Vec<_>>()
                    .into();
                row["complete"] = true.into();
                if cold {
                    row["construction_steps"] = construction_steps.into();
                }
            } else if cold {
                row["construction_steps"] = construction_steps.into();
                row["phase_evaluations"] = phase_evaluations.into();
                row["complete"] = true.into();
            }
            println!("{row}");
        }
    }
    Ok(())
}
