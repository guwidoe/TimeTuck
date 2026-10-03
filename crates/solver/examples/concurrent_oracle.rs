//! Public comparison using attributed concurrent_support; original inputs.
mod concurrent_support;
use concurrent_support::{Schedule, Visit};
use std::{
    error::Error,
    io::{self, BufRead},
};
use timetuck::*;
type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn parse(s: &str) -> Result<Vec<usize>> {
    if s == "_" {
        Ok(vec![])
    } else {
        s.split(',').map(|s| Ok(s.parse()?)).collect()
    }
}
fn row<T: ToString>(values: impl IntoIterator<Item = Option<T>>) -> String {
    let parts = values
        .into_iter()
        .map(|v| v.map_or_else(|| "-".into(), |v| v.to_string()))
        .collect::<Vec<_>>();
    if parts.is_empty() {
        "_".into()
    } else {
        parts.join(",")
    }
}
fn pos(e: &str, i: &str) -> Result<ListPosition> {
    Ok(ListPosition {
        entity: e.parse()?,
        index: i.parse()?,
    })
}
fn movement(s: &str) -> Result<ListMove<usize>> {
    let mut moves = vec![];
    for op in s.split(';').filter(|s| !s.is_empty()) {
        let bits = op.split(',').collect::<Vec<_>>();
        moves.push(match bits.as_slice() {
            ["a", v, e, i] => ListMove::Assign {
                value: v.parse()?,
                destination: pos(e, i)?,
            },
            ["c", e, i, d, j] => ListMove::Change {
                source: pos(e, i)?,
                destination: pos(d, j)?,
            },
            ["s", e, i, d, j] => ListMove::Swap {
                left: pos(e, i)?,
                right: pos(d, j)?,
            },
            _ => return Err("move".into()),
        });
    }
    Ok(ListMove::Composite(moves))
}
fn emit(
    id: &str,
    state: usize,
    a: &ListAssignment<usize>,
    g: &StreamGraph<'_, HardSoftScore>,
    handle: &StreamConstraint<(), Vec<Visit>>,
    s: &Schedule,
) {
    let matches = g.constraint_matches(handle).unwrap();
    let visits = &matches[&()].0;
    assert_eq!(visits, &s.project(a.lists()).0);
    assert_eq!(g.score(), s.score(a.lists()));
    assert_eq!(g.score(), s.reference_score(a.lists()));
    let lists = a
        .lists()
        .iter()
        .map(|vs| row(vs.iter().copied().map(Some)))
        .collect::<Vec<_>>()
        .join(";");
    let mut fields = vec![
        id.to_string(),
        state.to_string(),
        lists,
        g.score().hard().to_string(),
        g.score().soft().to_string(),
    ];
    fields.push(row(visits.iter().map(|v| v.owner)));
    fields.push(row(visits.iter().map(|v| v.index)));
    fields.push(row(visits.iter().map(|v| v.previous)));
    fields.push(row(visits.iter().map(|v| v.next)));
    fields.push(row(visits.iter().map(|v| v.cascading)));
    fields.push(row(visits.iter().map(|v| v.ready)));
    fields.push(row(visits.iter().map(|v| v.start)));
    fields.push(row(visits.iter().map(|v| v.finish)));
    fields.push(row(visits
        .iter()
        .map(|v| Some(usize::from(v.inconsistent)))));
    println!("{}", fields.join("\t"));
}
fn main() -> Result<()> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        let f = line.split('\t').collect::<Vec<_>>();
        if f.len() != 5 {
            return Err("five fields".into());
        }
        let n = f[1].parse::<usize>()?;
        if n > 64 {
            return Err("size".into());
        }
        let groups = if f[2] == "_" {
            vec![]
        } else {
            f[2].split(';').map(parse).collect::<Result<Vec<_>>>()?
        };
        let lists = f[3].split(';').map(parse).collect::<Result<Vec<_>>>()?;
        if lists.is_empty() || lists.len() > 64 {
            return Err("owner count".into());
        }
        let s = Schedule::new(&groups, n);
        let (g, sources, handle) = s.graph(lists.len());
        let p = ListProblem::new(
            (0..n).collect(),
            lists.into_iter().map(ListEntity::new).collect(),
        )?;
        let mut d = ListStreamScoreDirector::new(&p, g, sources, p.initial_assignment())?;
        let mut state = 0;
        emit(f[0], state, d.assignment(), d.graph(), &handle, &s);
        state += 1;
        for batch in f[4].split('|').filter(|s| !s.is_empty()) {
            let (mode, ops) = batch.split_once('@').ok_or("mode")?;
            if !matches!(mode, "c" | "t") {
                return Err("mode".into());
            }
            let t = d.apply(&movement(ops)?)?;
            emit(f[0], state, t.assignment(), t.graph(), &handle, &s);
            state += 1;
            if mode == "c" {
                t.commit()
            } else {
                drop(t);
                emit(f[0], state, d.assignment(), d.graph(), &handle, &s);
                state += 1;
            }
            d.assert_consistent()?;
        }
    }
    Ok(())
}
