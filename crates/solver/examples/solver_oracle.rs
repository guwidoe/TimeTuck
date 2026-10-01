//! Original complete-planning oracle; not a user-facing solver CLI.
mod solver_support;
use std::{
    error::Error,
    io::{self, BufRead},
};
use timetuck::exhaustive_search;

fn main() -> Result<(), Box<dyn Error>> {
    for line in io::stdin().lock().lines() {
        let case = solver_support::Case::parse(&line?)?;
        let budget: u64 = case
            .problem
            .search_space_size()
            .ok_or("space overflow")?
            .try_into()?;
        let result = exhaustive_search(&case.problem, |state| case.score(state), budget);
        if !result.is_optimal() {
            return Err("expected a complete optimal result".into());
        }
        let best = result.best.ok_or("no result")?;
        if case.score(&best.assignment) != best.score {
            return Err("score recomputation mismatch".into());
        }
        let values = best
            .assignment
            .values()
            .iter()
            .map(|v| v.map_or_else(|| "-".to_owned(), |v| v.to_string()))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{}\t{}\t{}\t{}",
            case.id,
            best.score.hard(),
            best.score.soft(),
            values
        );
    }
    Ok(())
}
