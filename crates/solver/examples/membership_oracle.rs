//! Original public-protocol comparison adapter; no upstream method credit.
use std::{
    error::Error,
    io::{self, BufRead},
};
use timetuck::*;
type Row = (i32, i32, Option<i32>, i64, Vec<Option<i32>>);
fn key(s: &str) -> Result<Option<i32>, std::num::ParseIntError> {
    if s == "n" {
        Ok(None)
    } else {
        s.parse().map(Some)
    }
}
fn encoded_key(k: &Option<i32>) -> String {
    k.map_or_else(|| "n".into(), |k| k.to_string())
}
fn encoded(r: &Row) -> String {
    format!(
        "{},{},{},{},{}",
        r.0,
        r.1,
        encoded_key(&r.2),
        r.3,
        r.4.iter().map(encoded_key).collect::<Vec<_>>().join("/")
    )
}
fn emit(
    id: &str,
    state: usize,
    g: &StreamGraph<'_, HardSoftScore>,
    terminal: &StreamConstraint<(usize, usize), (Row, Row)>,
) -> Result<(), GraphError> {
    let score = g.score();
    let matches = g
        .constraint_matches(terminal)?
        .iter()
        .map(|((i, j), ((a, b), _))| format!("{i},{j},{}|{}", encoded(a), encoded(b)))
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
        if mode >= 9 {
            return Err("mode".into());
        }
        let mut b = StreamGraphBuilder::<HardSoftScore>::new();
        let l = b.source::<usize, Row>();
        let r = b.source::<usize, Row>();
        let joined = if mode == 0 {
            b.join_containing(l.port(), r.port(), |r| r.4.clone(), |r| r.2)?
        } else if mode == 1 {
            b.join_contained_in(l.port(), r.port(), |r| r.2, |r| r.4.clone())?
        } else if mode == 2 {
            b.join_containing_any_of(l.port(), r.port(), |r| r.4.clone(), |r| r.4.clone())?
        } else {
            let partition = move |r: &Row| (r.0, if mode < 6 { 0 } else { r.1 });
            b.join_containing_any_of_partitioned(
                l.port(),
                r.port(),
                move |r| {
                    (
                        partition(r),
                        if mode % 3 == 1 {
                            vec![r.2]
                        } else {
                            r.4.clone()
                        },
                    )
                },
                move |r| {
                    (
                        partition(r),
                        if mode.is_multiple_of(3) {
                            vec![r.2]
                        } else {
                            r.4.clone()
                        },
                    )
                },
            )?
        };
        let terminal = b.score("matches", &joined, |(a, b)| {
            HardSoftScore::new(1, a.3 * b.3)
        })?;
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
                if row.len() != 5 {
                    return Err("row arity".into());
                }
                let weight: i64 = row[3].parse()?;
                if !(-1000..=1000).contains(&weight) {
                    return Err("weight bound".into());
                }
                Some((
                    row[0].parse()?,
                    row[1].parse()?,
                    key(row[2])?,
                    weight,
                    if row[4].is_empty() {
                        vec![]
                    } else {
                        row[4].split('/').map(key).collect::<Result<Vec<_>, _>>()?
                    },
                ))
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
