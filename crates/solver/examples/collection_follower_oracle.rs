//! Generated public comparison adapter. Domain/supplier fixtures attributed in
//! collection_follower_support and compat/provenance.json; inputs are original.
mod collection_follower_support;
use collection_follower_support::{Shadow, graph};
use std::{
    error::Error,
    io::{self, BufRead},
};
use timetuck::*;
fn parse(s: &str) -> Result<Vec<Option<usize>>, Box<dyn Error>> {
    if s == "_" {
        return Ok(vec![]);
    }
    s.split(',')
        .map(|v| {
            if v == "-" {
                Ok(None)
            } else {
                Ok(Some(v.parse()?))
            }
        })
        .collect()
}
fn row(values: &[Option<usize>]) -> String {
    if values.is_empty() {
        return "_".into();
    }
    values
        .iter()
        .map(|v| v.map_or_else(|| "-".into(), |v| v.to_string()))
        .collect::<Vec<_>>()
        .join(",")
}
fn emit(
    id: &str,
    state: usize,
    a: &Assignment<usize>,
    g: &StreamGraph<'_, SimpleScore>,
    shadow: &Shadow,
) -> Result<(), GraphError> {
    let m = g.constraint_matches(shadow)?;
    let derived: Vec<_> = m.values().map(|(v, _)| v.1).collect();
    println!(
        "{}\t{}\t{}\t{}\t{}\t{}",
        id,
        state,
        usize::from(a.values().iter().all(Option::is_some)),
        g.score().score(),
        row(a.values()),
        row(&derived)
    );
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        let f: Vec<_> = line.split('\t').collect();
        if f.len() != 5 {
            return Err("five fields".into());
        }
        let codes: Vec<String> = f[1].split(',').map(str::to_owned).collect();
        let initial = parse(f[2])?;
        if codes.len() > 32 || !(1..=64).contains(&initial.len()) {
            return Err("size bound".into());
        }
        let mut dependencies = vec![];
        if f[3] != "_" {
            for group in f[3].split(';') {
                let refs = parse(group)?
                    .into_iter()
                    .map(|r| r.ok_or("null member"))
                    .collect::<Result<Vec<_>, _>>()?;
                if refs.is_empty() || refs.iter().any(|&r| r >= initial.len()) {
                    return Err("dependency collection".into());
                }
                dependencies.push(refs);
            }
        }
        if dependencies.len() > 64 {
            return Err("follower count".into());
        }
        let p = FiniteProblem::new(
            initial
                .iter()
                .map(|v| {
                    let var = Variable::new((0..codes.len()).collect());
                    v.map_or(var.clone(), |v| var.with_initial(v))
                })
                .collect(),
        )?;
        let (g, s, shadow) = graph(&dependencies, &codes);
        let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment())?;
        let mut state = 0;
        emit(f[0], state, d.assignment(), d.graph(), &shadow)?;
        state += 1;
        for batch in f[4].split('|').filter(|s| !s.is_empty()) {
            let parts: Vec<_> = batch.split('@').collect();
            if parts.len() != 2 || !matches!(parts[0], "t" | "c") {
                return Err("mode".into());
            }
            let mut moves = vec![];
            for op in parts[1].split(';').filter(|s| !s.is_empty()) {
                let bits: Vec<_> = op.split(',').collect();
                if bits.len() != 2 {
                    return Err("operation".into());
                }
                moves.push(Move::Change {
                    variable: bits[0].parse()?,
                    value: if bits[1] == "-" {
                        None
                    } else {
                        Some(bits[1].parse()?)
                    },
                });
            }
            let t = d.apply(&Move::Composite(moves))?;
            emit(f[0], state, t.assignment(), t.graph(), &shadow)?;
            state += 1;
            if parts[0] == "c" {
                t.commit();
            } else {
                drop(t);
                emit(f[0], state, d.assignment(), d.graph(), &shadow)?;
                state += 1;
            }
            d.assert_consistent()?;
        }
    }
    Ok(())
}
