//! Public comparison adapter using attributed list_element_support fixtures.
//! Generated inputs are original and earn no upstream method credit.
mod list_element_support;
use list_element_support as support;
use std::{
    error::Error,
    io::{self, BufRead},
};
use timetuck::*;
type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn parse(s: &str) -> Result<Vec<Option<i32>>> {
    if s == "_" {
        return Ok(vec![]);
    }
    s.split(',')
        .map(|v| Ok(if v == "-" { None } else { Some(v.parse()?) }))
        .collect()
}
fn row<T: ToString>(vs: impl IntoIterator<Item = Option<T>>) -> String {
    let parts = vs
        .into_iter()
        .map(|v| v.map_or_else(|| "-".into(), |v| v.to_string()))
        .collect::<Vec<_>>();
    if parts.is_empty() {
        "_".into()
    } else {
        parts.join(",")
    }
}
fn lists(a: &ListAssignment<usize>) -> String {
    a.lists()
        .iter()
        .map(|l| row(l.iter().copied().map(Some)))
        .collect::<Vec<_>>()
        .join(";")
}
fn pos(e: &str, i: &str) -> Result<ListPosition> {
    Ok(ListPosition {
        entity: e.parse()?,
        index: i.parse()?,
    })
}
fn movement(s: &str) -> Result<MixedMove<i32, usize>> {
    let (mut basic, mut lists) = (vec![], vec![]);
    for op in s.split(';').filter(|s| !s.is_empty()) {
        let bits = op.split(',').collect::<Vec<_>>();
        match bits.as_slice() {
            ["b", i, v] => basic.push(Move::Change {
                variable: i.parse()?,
                value: if *v == "-" { None } else { Some(v.parse()?) },
            }),
            ["a", v, e, i] => lists.push(ListMove::Assign {
                value: v.parse()?,
                destination: pos(e, i)?,
            }),
            ["c", e, i, d, j] => lists.push(ListMove::Change {
                source: pos(e, i)?,
                destination: pos(d, j)?,
            }),
            ["s", e, i, d, j] => lists.push(ListMove::Swap {
                left: pos(e, i)?,
                right: pos(d, j)?,
            }),
            ["u", e, i, j] => lists.push(ListMove::Unassign {
                entity: e.parse()?,
                start: i.parse()?,
                end: j.parse()?,
            }),
            _ => return Err("move".into()),
        }
    }
    Ok(MixedMove {
        basic: Move::Composite(basic),
        lists: ListMove::Composite(lists),
    })
}
fn ordered_emit(
    id: &str,
    state: usize,
    a: &ListAssignment<usize>,
    g: &StreamGraph<'_, SimpleScore>,
    c: &support::Ends,
    v: &support::Elements,
    durations: &[i32],
) {
    let times = g
        .constraint_matches(v)
        .unwrap()
        .values()
        .map(|(t, _)| t)
        .collect::<Vec<_>>();
    let ends = g.constraint_matches(c).unwrap();
    println!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        id,
        state,
        lists(a),
        row(durations.iter().copied().map(Some)),
        row(times.iter().map(|t| t.owner)),
        row(times.iter().map(|t| t.previous)),
        row(times.iter().map(|t| t.start)),
        row(times.iter().map(|t| t.end)),
        row(ends.values().map(|(v, _)| Some(v.1))),
        g.score().score()
    );
}
fn mixed_emit(
    id: &str,
    state: usize,
    a: &MixedAssignment<i32, usize>,
    g: &StreamGraph<'_, SimpleScore>,
    c: &support::Totals,
    v: &support::Totals,
) {
    let padded = g.constraint_matches(v).unwrap();
    let totals = g.constraint_matches(c).unwrap();
    println!(
        "{}\t{}\t{}\t{}\t_\t_\t_\t{}\t{}\t0",
        id,
        state,
        lists(a.lists()),
        row(a.basic().values().iter().copied()),
        row(padded.values().map(|(v, _)| v.1)),
        row(totals.values().map(|(v, _)| v.1))
    );
}
fn main() -> Result<()> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        let f = line.split('\t').collect::<Vec<_>>();
        if f.len() != 6 {
            return Err("six fields".into());
        }
        let starts = parse(f[2])?
            .into_iter()
            .map(|v| v.ok_or("null start"))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let durations = parse(f[3])?;
        let initial = f[4]
            .split(';')
            .map(|s| {
                parse(s)?
                    .into_iter()
                    .map(|v| Ok(usize::try_from(v.ok_or("null list member")?)?))
                    .collect::<Result<Vec<_>>>()
            })
            .collect::<Result<Vec<_>>>()?;
        if starts.len() != initial.len()
            || starts.is_empty()
            || starts.len() > 64
            || durations.len() > 64
        {
            return Err("shape".into());
        }
        let p = ListProblem::new(
            (0..durations.len()).collect(),
            initial.into_iter().map(ListEntity::new).collect(),
        )?;
        let batches = f[5]
            .split('|')
            .filter(|s| !s.is_empty())
            .map(|s| {
                let (mode, ops) = s.split_once('@').ok_or("mode")?;
                if !matches!(mode, "t" | "c") {
                    return Err("mode".into());
                }
                Ok((mode, movement(ops)?))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut state = 0;
        match f[1] {
            "o" => {
                let durations = durations
                    .into_iter()
                    .map(|v| v.ok_or("null fixed duration"))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                let p = p.allowing_unassigned();
                let (g, s, c, v) = support::ordered(starts, durations.clone());
                let mut d = ListStreamScoreDirector::new(&p, g, s, p.initial_assignment())?;
                ordered_emit(f[0], state, d.assignment(), d.graph(), &c, &v, &durations);
                state += 1;
                for (mode, m) in batches {
                    if !matches!(&m.basic,Move::Composite(v) if v.is_empty()) {
                        return Err("fixed duration change".into());
                    }
                    let t = d.apply(&m.lists)?;
                    ordered_emit(f[0], state, t.assignment(), t.graph(), &c, &v, &durations);
                    state += 1;
                    if mode == "c" {
                        t.commit()
                    } else {
                        drop(t);
                        ordered_emit(f[0], state, d.assignment(), d.graph(), &c, &v, &durations);
                        state += 1;
                    }
                    d.assert_consistent()?;
                }
            }
            "m" => {
                let basic = FiniteProblem::new(
                    durations
                        .into_iter()
                        .map(|v| {
                            let var = Variable::new((1..=9).collect()).allowing_unassigned();
                            v.map_or(var.clone(), |v| var.with_initial(v))
                        })
                        .collect(),
                )?;
                let p = MixedProblem::new(basic, p);
                let (g, s, c, v) = support::mixed();
                let mut d = MixedStreamScoreDirector::new(&p, g, s, p.initial_assignment())?;
                mixed_emit(f[0], state, d.assignment(), d.graph(), &c, &v);
                state += 1;
                for (mode, m) in batches {
                    let t = d.apply(&m)?;
                    mixed_emit(f[0], state, t.assignment(), t.graph(), &c, &v);
                    state += 1;
                    if mode == "c" {
                        t.commit()
                    } else {
                        drop(t);
                        mixed_emit(f[0], state, d.assignment(), d.graph(), &c, &v);
                        state += 1;
                    }
                    d.assert_consistent()?;
                }
            }
            _ => return Err("model".into()),
        }
    }
    Ok(())
}
