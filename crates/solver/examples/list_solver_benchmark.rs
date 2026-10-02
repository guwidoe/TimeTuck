//! Original full-scoring list-solving comparison adapter; no Java trace assumption.
mod list_solver_support;
use list_solver_support::Case;
use std::{
    error::Error,
    hint::black_box,
    io::{self, BufRead},
    time::Instant,
};
use timetuck::*;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 5 || !["construct", "hill"].contains(&args[1].as_str()) {
        return Err("construct|hill steps warmup samples".into());
    }
    let steps: u64 = args[2].parse()?;
    let warmup: usize = args[3].parse()?;
    let samples: usize = args[4].parse()?;
    if steps == 0 || samples == 0 {
        return Err("positive counts required".into());
    }
    for line in io::stdin().lock().lines() {
        let case = Case::parse(&line?)?;
        if args[1] == "hill" && !case.problem.is_complete(&case.problem.initial_assignment()) {
            return Err("complete hill input required".into());
        }
        let mut network = ListConstraintNetwork::new(case.depots.len(), case.points.len());
        network.add_constraint(
            "full score",
            (0..case.depots.len()).map(ListScope::Entity).collect(),
            |f| {
                case.score_lists(f.iter().map(|v| match v {
                    ListFact::Entity(list) => *list,
                    _ => unreachable!(),
                }))
            },
        )?;
        let scan_bound = (case.points.len() as u64)
            .checked_mul((case.points.len() + case.depots.len()) as u64)
            .ok_or("budget overflow")?;
        for run in 0..warmup.checked_add(samples).ok_or("sample overflow")? {
            let start = Instant::now();
            let (best, calls) = if args[1] == "construct" {
                let r = greedy_list_construction(
                    black_box(&case.problem),
                    &network,
                    ConstructionConfig {
                        max_evaluations: scan_bound + 1,
                        ..Default::default()
                    },
                )?;
                if r.termination != ConstructionTermination::Completed {
                    return Err("construction did not finish".into());
                }
                (
                    r.solution().ok_or("no complete result")?,
                    r.constraint_evaluations,
                )
            } else {
                let r = list_local_search_from(
                    black_box(&case.problem),
                    &network,
                    case.problem.initial_assignment(),
                    LocalSearchConfig {
                        max_evaluations: steps
                            .checked_mul(scan_bound)
                            .and_then(|x| x.checked_add(1))
                            .ok_or("budget overflow")?,
                        max_steps: steps,
                        max_accepted_moves: u64::MAX,
                        acceptance: Acceptance::HillClimbing,
                        neighborhood: Neighborhood::Change,
                        selection: MoveSelection::BestAccepted,
                        rejected_moves: RejectedMovePolicy::BestScore,
                        ..Default::default()
                    },
                )?;
                (r.best.ok_or("no result")?, r.constraint_evaluations)
            };
            case.problem.validate(&best.assignment)?;
            if !case.problem.is_complete(&best.assignment)
                || case.score(&best.assignment) != best.score
            {
                return Err("invalid result".into());
            }
            // Exercise derived shadows and their consistency, as in the Java adapter.
            for (e, list) in best.assignment.lists().iter().enumerate() {
                for (i, value) in list.iter().enumerate() {
                    let s = best.assignment.element(value).ok_or("missing shadow")?;
                    if s.position
                        != (ListPosition {
                            entity: e,
                            index: i,
                        })
                        || s.previous != i.checked_sub(1).and_then(|j| list.get(j))
                        || s.next != list.get(i + 1)
                    {
                        return Err("shadow mismatch".into());
                    }
                }
            }
            let elapsed = start.elapsed().as_nanos();
            if run >= warmup {
                println!(
                    "{}\t{elapsed}\t{}\t{}\t{}\t{calls}",
                    case.id,
                    best.score.hard(),
                    best.score.soft(),
                    Case::encode_assignment(&best.assignment)
                );
            }
        }
    }
    Ok(())
}
