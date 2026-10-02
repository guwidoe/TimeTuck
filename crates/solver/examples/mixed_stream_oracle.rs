//! Original mixed move/score adapter; no upstream method credit or timing claim.
use std::{
    error::Error,
    io::{self, BufRead},
};
use timetuck::*;
fn option(s: &str) -> Result<Option<i32>, Box<dyn Error>> {
    Ok(if s == "-" { None } else { Some(s.parse()?) })
}
fn position(e: &str, i: &str) -> Result<ListPosition, Box<dyn Error>> {
    Ok(ListPosition {
        entity: e.parse()?,
        index: i.parse()?,
    })
}
fn emit(
    id: &str,
    state: usize,
    score: HardSoftScore,
    a: &MixedAssignment<i32, i32>,
    g: &StreamGraph<'_, HardSoftScore>,
    s: &MixedStreamSources<i32, i32>,
    values: &[i32],
) {
    let basic = g.source_rows(&s.basic).unwrap();
    assert_eq!(basic.len(), a.basic().values().len());
    for (i, v) in a.basic().values().iter().enumerate() {
        assert_eq!(&basic[&i], &(i, *v));
    }
    let entities = g.source_rows(&s.lists.entities).unwrap();
    assert_eq!(entities.len(), a.lists().lists().len());
    for (i, v) in a.lists().lists().iter().enumerate() {
        assert_eq!(&entities[&i], &(i, v.clone()));
    }
    let views = g.source_rows(&s.lists.values).unwrap();
    assert_eq!(views.len(), values.len());
    let shadows = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let r = &views[&i];
            assert_eq!(r.value, *v);
            assert_eq!(r.range_index, i);
            r.position.map_or_else(
                || {
                    assert!(
                        r.previous.is_none() && r.next.is_none(),
                        "unassigned value retains neighbors"
                    );
                    "-".to_owned()
                },
                |p| {
                    format!(
                        "{}:{}:{}:{}",
                        p.entity,
                        p.index,
                        r.previous.map_or_else(|| "-".to_owned(), |v| v.to_string()),
                        r.next.map_or_else(|| "-".to_owned(), |v| v.to_string())
                    )
                },
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let basics = a
        .basic()
        .values()
        .iter()
        .map(|v| v.map_or_else(|| "-".to_owned(), |v| v.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    let lists = a
        .lists()
        .lists()
        .iter()
        .map(|v| {
            if v.is_empty() {
                "-".to_owned()
            } else {
                v.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            }
        })
        .collect::<Vec<_>>()
        .join(";");
    let missing = a.basic().values().iter().filter(|v| v.is_none()).count();
    assert_eq!(score.structural(), -(missing as i64));
    assert_eq!(score.with_structural(0), g.score());
    println!(
        "{id}\t{state}\t{}\t{}\t{}\t{basics}\t{lists}\t{shadows}",
        usize::from(missing == 0),
        score.hard(),
        score.soft()
    );
}
fn main() -> Result<(), Box<dyn Error>> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        let f: Vec<_> = line.split('\t').collect();
        if f.len() != 6 {
            return Err("six fields required".into());
        }
        let initial = f[1].split(',').map(option).collect::<Result<Vec<_>, _>>()?;
        let pins = f[2]
            .split(',')
            .map(|v| match v {
                "0" => Ok(false),
                "1" => Ok(true),
                _ => Err("pin flag"),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let values: Vec<i32> = if f[3] == "-" {
            vec![]
        } else {
            f[3].split(',').map(str::parse).collect::<Result<_, _>>()?
        };
        let lists = f[4]
            .split(';')
            .map(|v| {
                if v == "-" {
                    Ok(vec![])
                } else {
                    v.split(',').map(str::parse).collect()
                }
            })
            .collect::<Result<Vec<Vec<i32>>, _>>()?;
        if initial.len() != pins.len() || initial.len() != lists.len() {
            return Err("shape mismatch".into());
        }
        let variables = initial
            .iter()
            .zip(pins)
            .map(|(v, pin)| {
                let x = Variable::new(vec![-2, 0, 3]).with_pinned(pin);
                if let Some(v) = v {
                    x.with_initial(*v)
                } else {
                    x
                }
            })
            .collect();
        let p = MixedProblem::new(
            FiniteProblem::new(variables)?,
            ListProblem::new(
                values.clone(),
                lists.into_iter().map(ListEntity::new).collect(),
            )?
            .allowing_unassigned(),
        );
        let mut b = StreamGraphBuilder::<HardSoftScore>::new();
        let sources = MixedStreamSources::<i32, i32>::new(&mut b);
        let inspect = sources.clone();
        b.score("capacity", sources.lists.entities.port(), |(_, list)| {
            HardSoftScore::new(-(list.len().saturating_sub(2) as i64), 0)
        })?;
        let loads = b.join_equal(
            sources.lists.entities.port(),
            sources.basic.port(),
            |(i, _)| *i,
            |(i, _)| *i,
        )?;
        b.score("loads", &loads, |((_, list), (_, coefficient))| {
            HardSoftScore::new(0, i64::from(coefficient.unwrap_or(0)) * list.len() as i64)
        })?;
        let assigned = b.filter(sources.lists.values.port(), |v| v.position.is_some())?;
        let shadows = b.join_equal(
            &assigned,
            sources.basic.port(),
            |v| v.position.unwrap().entity,
            |(i, _)| *i,
        )?;
        b.score("shadows", &shadows, |(v, (_, coefficient))| {
            HardSoftScore::new(
                0,
                i64::from(coefficient.unwrap_or(0)) * i64::from(v.value)
                    + v.position.unwrap().index as i64
                    + i64::from(v.previous.unwrap_or(0))
                    - i64::from(v.next.unwrap_or(0)),
            )
        })?;
        let mut d = MixedStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment())?;
        let mut state = 0;
        emit(
            f[0],
            state,
            d.score(),
            d.assignment(),
            d.graph(),
            &inspect,
            &values,
        );
        state += 1;
        for batch in f[5].split('|').filter(|v| !v.is_empty()) {
            let (mode, ops) = batch.split_once('@').ok_or("batch mode")?;
            if !matches!(mode, "t" | "c") {
                return Err("batch mode".into());
            }
            let (mut basic, mut lists) = (vec![], vec![]);
            for op in ops.split(';').filter(|v| !v.is_empty()) {
                let parts: Vec<_> = op.split(',').collect();
                match parts.as_slice() {
                    ["b", i, v] => basic.push(Move::Change {
                        variable: i.parse()?,
                        value: option(v)?,
                    }),
                    ["a", v, e, i] => lists.push(ListMove::Assign {
                        value: v.parse()?,
                        destination: position(e, i)?,
                    }),
                    ["c", e, i, d, j] => lists.push(ListMove::Change {
                        source: position(e, i)?,
                        destination: position(d, j)?,
                    }),
                    ["s", e, i, d, j] => lists.push(ListMove::Swap {
                        left: position(e, i)?,
                        right: position(d, j)?,
                    }),
                    ["u", e, i, j] => lists.push(ListMove::Unassign {
                        entity: e.parse()?,
                        start: i.parse()?,
                        end: j.parse()?,
                    }),
                    _ => return Err("move kind".into()),
                }
            }
            let trial = d.apply(&MixedMove {
                basic: Move::Composite(basic),
                lists: ListMove::Composite(lists),
            })?;
            if mode == "c" {
                trial.commit();
            } else {
                emit(
                    f[0],
                    state,
                    trial.score(),
                    trial.assignment(),
                    trial.graph(),
                    &inspect,
                    &values,
                );
                state += 1;
                drop(trial);
            }
            emit(
                f[0],
                state,
                d.score(),
                d.assignment(),
                d.graph(),
                &inspect,
                &values,
            );
            state += 1;
            d.assert_consistent()?;
        }
    }
    Ok(())
}
