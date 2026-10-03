//! Original public-protocol comparison adapter; no upstream method credit.
use std::{
    error::Error,
    io::{self, BufRead},
};
use timetuck::*;
type Row = (i32, i32, i32, i64);
fn emit(
    id: &str,
    state: usize,
    g: &StreamGraph<'_, HardSoftScore>,
    terminal: &StreamConstraint<usize, Row>,
) -> Result<(), GraphError> {
    let score = g.score();
    let matches = g
        .constraint_matches(terminal)?
        .iter()
        .map(|(i, (a, _))| format!("{i},{},{},{},{}", a.0, a.1, a.2, a.3))
        .collect::<Vec<_>>()
        .join(";");
    println!(
        "{id}\t{state}\t{}\t{}\t{matches}",
        score.hard(),
        score.soft()
    );
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err("expected case,mode,operations".into());
        }
        let mode: usize = fields[1].parse()?;
        if mode >= 6 {
            return Err("mode".into());
        }
        let mut b = StreamGraphBuilder::<HardSoftScore>::new();
        let l = b.source::<usize, Row>();
        let r = b.source::<usize, Row>();
        let project = move |row: &Row| {
            (
                if mode < 2 { 0 } else { row.0 },
                if mode < 4 { 0 } else { row.1 },
            )
        };
        let retained = b.existence_equal(
            l.port(),
            r.port(),
            if mode.is_multiple_of(2) {
                ExistenceMode::Exists
            } else {
                ExistenceMode::NotExists
            },
            project,
            project,
        )?;
        let terminal = b.score("retained", &retained, |r| HardSoftScore::new(1, r.3))?;
        let mut g = b.build();
        let mut state = 0;
        emit(fields[0], state, &g, &terminal)?;
        state += 1;
        if fields[2].is_empty() {
            continue;
        }
        for op in fields[2].split(';') {
            let f = op.split(':').collect::<Vec<_>>();
            if f.len() != 4 || !matches!(f[0], "c" | "t") || !matches!(f[1], "L" | "R") {
                return Err("operation".into());
            }
            let id: usize = f[2].parse()?;
            if id >= 16 {
                return Err("id bound".into());
            }
            let value = if f[3] == "-" {
                None
            } else {
                let row = f[3].split(',').collect::<Vec<_>>();
                if row.len() != 4 {
                    return Err("row arity".into());
                }
                let weight: i64 = row[3].parse()?;
                if !(-1000..=1000).contains(&weight) {
                    return Err("weight bound".into());
                }
                Some((row[0].parse()?, row[1].parse()?, row[2].parse()?, weight))
            };
            let mut t = g.transaction();
            t.apply(if f[1] == "L" { &l } else { &r }, TupleChange { id, value })?;
            emit(fields[0], state, t.graph()?, &terminal)?;
            state += 1;
            if f[0] == "c" {
                t.commit()?;
            } else {
                drop(t);
                emit(fields[0], state, &g, &terminal)?;
                state += 1;
            }
            g.assert_consistent()?;
        }
    }
    Ok(())
}
