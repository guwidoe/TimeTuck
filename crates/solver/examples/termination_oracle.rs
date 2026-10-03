//! Original deterministic termination comparison, not a runtime benchmark.
mod solver_support;
use std::{
    error::Error,
    io::{self, BufRead},
};
use timetuck::*;

fn main() -> Result<(), Box<dyn Error>> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        let f: Vec<_> = line.split('\t').collect();
        if f.len() != 10 {
            return Err("expected ten fields".into());
        }
        let case = solver_support::Case::parse(&f[..6].join("\t"), true)?;
        if !case.problem.is_complete(&case.problem.initial_assignment()) {
            return Err("complete initial state required".into());
        }
        let c = LocalSearchConfig {
            max_steps: f[6].parse()?,
            max_unimproved_steps: if f[7] == "-" {
                None
            } else {
                Some(f[7].parse()?)
            },
            best_score_limit: if f[8] == "-" {
                None
            } else {
                Some(f[8].parse::<HardSoftScore>()?)
            },
            stop_when_feasible: match f[9] {
                "0" => false,
                "1" => true,
                _ => return Err("feasibility flag".into()),
            },
            max_evaluations: u64::MAX,
            max_accepted_moves: u64::MAX,
            acceptance: Acceptance::HillClimbing,
            neighborhood: Neighborhood::Change,
            selection: MoveSelection::BestAccepted,
            rejected_moves: RejectedMovePolicy::BestScore,
            assert_scores: true,
            ..Default::default()
        };
        let mut n = ConstraintNetwork::new(case.problem.variables().len());
        n.add_constraint("full", (0..case.problem.variables().len()).collect(), |v| {
            case.score_by(|i| v[i].copied())
        })?;
        let full = local_search(&case.problem, &n, c)?;
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<usize>)>();
        b.score("preferences", s.port(), |(i, v)| {
            HardSoftScore::new(0, case.weights[*i][v.unwrap()])
        })?;
        let pairs = b.join_equal(s.port(), s.port(), |_| (), |_| ())?;
        b.score("conflicts", &pairs, |((a, av), (z, zv))| {
            HardSoftScore::new(
                -if av == zv {
                    case.edges
                        .iter()
                        .filter(|&&(x, y)| x == *a && y == *z)
                        .count() as i64
                } else {
                    0
                },
                0,
            )
        })?;
        let stream = stream_local_search_from(
            &case.problem,
            b.build(),
            s,
            case.problem.initial_assignment(),
            c,
        )?;
        for (backend, r) in [("scoped", full), ("stream", stream)] {
            let best = r.best.ok_or("missing result")?;
            if case.score(&best.assignment) != best.score
                || !case.problem.is_complete(&best.assignment)
            {
                return Err("invalid returned score/state".into());
            }
            let values = best
                .assignment
                .values()
                .iter()
                .map(|v| v.unwrap().to_string())
                .collect::<Vec<_>>()
                .join(",");
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}\t{}",
                case.id,
                backend,
                best.score.hard(),
                best.score.soft(),
                values,
                r.completed_steps,
                r.unimproved_steps,
                r.termination,
                r.accepted_moves
            );
        }
    }
    Ok(())
}
