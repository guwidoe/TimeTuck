//! Original committed/trial stream update oracle; no upstream method credit.
mod solver_support;
mod stream_support;
use std::{
    error::Error,
    io::{self, BufRead},
};
use timetuck::*;
fn main() -> Result<(), Box<dyn Error>> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 7 {
            return Err("expected seven fields".into());
        }
        let case = solver_support::Case::parse(&fields[..6].join("\t"), true)?;
        let n = stream_support::build(&case)?;
        let mut d = StreamScoreDirector::new(
            &case.problem,
            n.graph,
            n.source,
            case.problem.initial_assignment(),
        )?;
        let mut state = 0;
        stream_support::emit(
            &case.id,
            state,
            d.graph(),
            d.assignment(),
            &n.conflicts,
            &n.preferences,
        )?;
        state += 1;
        for operation in fields[6].split(';').filter(|s| !s.is_empty()) {
            let bits: Vec<_> = operation.split(':').collect();
            if bits.len() != 3 || !matches!(bits[0], "t" | "c") {
                return Err("invalid operation".into());
            }
            let variable = bits[1].parse()?;
            let value = if bits[2] == "-" {
                None
            } else {
                Some(bits[2].parse()?)
            };
            let candidate = d.apply(&Move::Change { variable, value })?;
            if stream_support::score(&case, candidate.assignment())
                != candidate.score().with_structural(0)
            {
                return Err("incorrect candidate".into());
            }
            stream_support::emit(
                &case.id,
                state,
                candidate.graph(),
                candidate.assignment(),
                &n.conflicts,
                &n.preferences,
            )?;
            state += 1;
            if bits[0] == "c" {
                candidate.commit();
            } else {
                drop(candidate);
                stream_support::emit(
                    &case.id,
                    state,
                    d.graph(),
                    d.assignment(),
                    &n.conflicts,
                    &n.preferences,
                )?;
                state += 1;
            }
            d.assert_consistent()?;
        }
    }
    Ok(())
}
