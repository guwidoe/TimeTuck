//! Original matched exact-search benchmark. Model parsing occurs before timing.
mod solver_support;
use std::{
    error::Error,
    hint::black_box,
    io::{self, BufRead},
    time::Instant,
};
use timetuck::exhaustive_search;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("expected iterations warmup samples".into());
    }
    let iterations: usize = args[1].parse()?;
    let warmup: usize = args[2].parse()?;
    let samples: usize = args[3].parse()?;
    if iterations == 0 || samples == 0 {
        return Err("positive iterations and samples required".into());
    }
    let line = io::stdin().lock().lines().next().ok_or("missing case")??;
    let case = solver_support::Case::parse(&line)?;
    black_box(&case.id);
    let budget = case
        .problem
        .search_space_size()
        .ok_or("space overflow")?
        .try_into()?;
    for sample in 0..warmup + samples {
        let (mut hard, mut soft) = (0_i64, 0_i64);
        let mut last = None;
        let started = Instant::now();
        for _ in 0..iterations {
            let result =
                exhaustive_search(black_box(&case.problem), |state| case.score(state), budget);
            if !result.is_optimal() {
                return Err("incomplete result".into());
            }
            let best = result.best.ok_or("missing result")?;
            if case.score(&best.assignment) != best.score
                || !case.problem.is_complete(&best.assignment)
            {
                return Err("invalid result".into());
            }
            hard += best.score.hard();
            soft += best.score.soft();
            last = Some(best);
        }
        let elapsed = started.elapsed().as_nanos();
        if sample >= warmup {
            let values = last
                .unwrap()
                .assignment
                .values()
                .iter()
                .map(|v| v.unwrap().to_string())
                .collect::<Vec<_>>()
                .join(",");
            println!("{elapsed}\t{hard}\t{soft}\t{values}");
        }
    }
    Ok(())
}
