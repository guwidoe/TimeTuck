//! Original single-route cyclic k-opt comparison adapter.
#[allow(dead_code)]
mod list_solver_support;
use list_solver_support::Case;
use std::{
    error::Error,
    io::{self, BufRead},
    time::Instant,
};
use timetuck::*;
fn score(case: &Case, row: &[usize]) -> HardSoftScore {
    let mut distance = 0;
    for i in 0..row.len() {
        let a = case.points[row[i]];
        let b = case.points[row[(i + 1) % row.len()]];
        distance += (a.0 - b.0).abs() + (a.1 - b.1).abs();
    }
    HardSoftScore::new(0, -distance)
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 8 {
        return Err("minimum maximum steps evaluations seed warmup samples".into());
    }
    let minimum: usize = args[1].parse()?;
    let k: usize = args[2].parse()?;
    let steps: u64 = args[3].parse()?;
    let evaluations: u64 = args[4].parse()?;
    let seed: u64 = args[5].parse()?;
    let warmup: usize = args[6].parse()?;
    let samples: usize = args[7].parse()?;
    if minimum < 2
        || minimum > k
        || k > 4
        || steps == 0
        || steps > i32::MAX as u64
        || evaluations == 0
        || evaluations > i64::MAX as u64
        || seed > i64::MAX as u64
        || samples == 0
    {
        return Err("counts".into());
    }
    for line in io::stdin().lock().lines() {
        let case = Case::parse(&line?)?;
        if case.problem.entities().len() != 1 {
            return Err("one entity required".into());
        }
        let e = &case.problem.entities()[0];
        let initial = case.problem.initial_assignment();
        if e.is_pinned()
            || e.pinned_prefix() != 0
            || case.points.len() < 2 * k
            || !case.problem.is_complete(&initial)
            || case.capacities[0] < case.points.len()
        {
            return Err("complete unpinned sufficiently long route required".into());
        }
        let mut network = ListConstraintNetwork::new(1, case.points.len());
        network.add_constraint("cyclic distance", vec![ListScope::Entity(0)], |f| {
            let ListFact::Entity(row) = f[0] else {
                unreachable!()
            };
            score(&case, row)
        })?;
        let initial_score = score(&case, &initial.lists()[0]);
        for run in 0..warmup.checked_add(samples).ok_or("sample overflow")? {
            let start = Instant::now();
            let result = list_local_search_from(
                &case.problem,
                &network,
                initial.clone(),
                LocalSearchConfig {
                    max_evaluations: evaluations,
                    max_steps: steps,
                    max_accepted_moves: u64::MAX,
                    acceptance: Acceptance::HillClimbing,
                    neighborhood: Neighborhood::KOpt(ListKOptConfig::new(minimum, k)?),
                    move_order: MoveOrder::ShuffledMoves {
                        seed,
                        window: 4096.try_into().unwrap(),
                    },
                    selection: MoveSelection::FirstAccepted,
                    rejected_moves: RejectedMovePolicy::BestScore,
                    ..Default::default()
                },
            )?;
            let calls = result.constraint_evaluations;
            let best = result.best.ok_or("no result")?;
            case.problem.validate(&best.assignment)?;
            if !case.problem.is_complete(&best.assignment)
                || score(&case, &best.assignment.lists()[0]) != best.score
                || best.score < initial_score
            {
                return Err("invalid score or assignment".into());
            }
            for (i, value) in best.assignment.lists()[0].iter().enumerate() {
                let shadow = best.assignment.element(value).ok_or("missing shadow")?;
                if shadow.position
                    != (ListPosition {
                        entity: 0,
                        index: i,
                    })
                    || shadow.previous
                        != i.checked_sub(1)
                            .and_then(|j| best.assignment.lists()[0].get(j))
                    || shadow.next != best.assignment.lists()[0].get(i + 1)
                {
                    return Err("invalid shadow".into());
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initial_shuffle_window_reaches_all_configured_edge_counts() {
        for n in [8, 12, 16] {
            let p = ListProblem::new((0..n).collect(), vec![ListEntity::new((0..n).collect())])
                .unwrap();
            let a = p.initial_assignment();
            let mut preceding = 0;
            for k in 2..=4 {
                assert!(preceding < 4096, "n={n}, k={k}, preceding={preceding}");
                let mut moves =
                    list_k_opt_moves(&p, &a, ListKOptConfig::new(k, k).unwrap()).unwrap();
                assert!(moves.next().is_some());
                if k < 4 {
                    preceding += 1 + moves.count();
                }
            }
        }
    }
}
