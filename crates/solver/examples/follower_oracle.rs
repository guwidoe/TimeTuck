//! Public-comparison development adapter. Domain/scoring fixture adapted from
//! Timefold Community testdomain/shadow/{dynamic_follower,follower}, Apache-2.0.
//! See compat/provenance.json; input provenance is recorded by follower_checks.py.
use std::{
    error::Error,
    io::{self, BufRead},
};
use timetuck::*;
type Shadow = StreamConstraint<usize, ((usize, Option<usize>), Option<usize>)>;
fn graph(
    leaders: usize,
    fixed: Option<&[Option<usize>]>,
) -> (
    StreamGraph<'static, SimpleScore>,
    PlanningStreamSource<usize>,
    Shadow,
) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (usize, Option<usize>)>();
    let targets = b.filter(s.port(), move |r| r.0 < leaders).unwrap();
    let facts = b.source::<usize, (usize, Option<usize>)>();
    let followers = if fixed.is_some() {
        facts.port().clone()
    } else {
        b.filter(s.port(), move |r| r.0 >= leaders).unwrap()
    };
    let values = if fixed.is_some() {
        b.follow_shared_reference(
            &followers,
            &targets,
            |r| r.1.unwrap_or(usize::MAX),
            |_, t| t.and_then(|r| r.1),
        )
        .unwrap()
    } else {
        let followed = b.follow_reference(&followers, &targets, |r| r.1).unwrap();
        b.map(&followed, |(f, t)| (*f, t.and_then(|t| t.1)))
            .unwrap()
    };
    let shadow = b.score("derived", &values, |_| SimpleScore::ZERO).unwrap();
    let follower_values = b.map(&values, |(_, v)| *v).unwrap();
    let leader_values = b.map(&targets, |r| r.1).unwrap();
    let all = b.concat(&leader_values, &follower_values).unwrap();
    let counts = b.group_collect(&all, |v| (*v, 1)).unwrap();
    b.score("Minimize value count", &counts, |(_, n)| {
        SimpleScore::new(-(n.count as i64).pow(2))
    })
    .unwrap();
    let mut g = b.build();
    if let Some(refs) = fixed {
        let mut t = g.transaction();
        for (i, &r) in refs.iter().enumerate() {
            t.apply(&facts, TupleChange::put(leaders + i, (leaders + i, r)))
                .unwrap();
        }
        t.commit().unwrap();
    }
    (g, s, shadow)
}

fn parse_list(s: &str) -> Result<Vec<Option<usize>>, Box<dyn Error>> {
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
fn list(values: &[Option<usize>]) -> String {
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
    leaders: usize,
    a: &Assignment<usize>,
    g: &StreamGraph<'_, SimpleScore>,
    shadow: &Shadow,
    fixed: Option<&[Option<usize>]>,
) -> Result<(), GraphError> {
    let m = g.constraint_matches(shadow)?;
    let values = a.values();
    let refs = fixed.unwrap_or(&values[leaders..]);
    let derived: Vec<_> = (leaders..leaders + refs.len()).map(|i| m[&i].0.1).collect();
    println!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}",
        id,
        state,
        usize::from(values.iter().all(Option::is_some)),
        g.score().score(),
        list(&values[..leaders]),
        list(refs),
        list(&derived)
    );
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        let f: Vec<_> = line.split('\t').collect();
        if f.len() != 6 {
            return Err("six fields".into());
        }
        let k: usize = f[1].parse()?;
        let mut initial = parse_list(f[2])?;
        let leaders = initial.len();
        let followers = parse_list(f[3])?;
        if !(1..=32).contains(&k) || !(1..=64).contains(&leaders) || followers.len() > 64 {
            return Err("size bound".into());
        }
        let shared = match f[5] {
            "shared" => true,
            "dynamic" => false,
            _ => return Err("model".into()),
        };
        if followers.iter().flatten().any(|&r| r >= leaders) {
            return Err("reference domain".into());
        }
        let fixed = shared.then_some(followers.as_slice());
        if !shared {
            initial.extend_from_slice(&followers);
        }
        let p = FiniteProblem::new(
            initial
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let var = Variable::new((0..if i < leaders { k } else { leaders }).collect());
                    v.map_or(var.clone(), |v| var.with_initial(v))
                })
                .collect(),
        )?;
        let (g, s, shadow) = graph(leaders, fixed);
        let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment())?;
        let mut state = 0;
        emit(
            f[0],
            state,
            leaders,
            d.assignment(),
            d.graph(),
            &shadow,
            fixed,
        )?;
        state += 1;
        for batch in f[4].split('|').filter(|s| !s.is_empty()) {
            let parts: Vec<_> = batch.split('@').collect();
            if parts.len() != 2 || !matches!(parts[0], "t" | "c") {
                return Err("mode".into());
            }
            let mut moves = vec![];
            for op in parts[1].split(';').filter(|s| !s.is_empty()) {
                let bits: Vec<_> = op.split(',').collect();
                if bits.len() != 3 {
                    return Err("operation".into());
                }
                let i: usize = bits[1].parse()?;
                let variable = match bits[0] {
                    "v" if i < leaders => i,
                    "r" if i < initial.len() - leaders => leaders + i,
                    _ => return Err("kind/index".into()),
                };
                let value = if bits[2] == "-" {
                    None
                } else {
                    Some(bits[2].parse()?)
                };
                moves.push(Move::Change { variable, value });
            }
            let t = d.apply(&Move::Composite(moves))?;
            emit(
                f[0],
                state,
                leaders,
                t.assignment(),
                t.graph(),
                &shadow,
                fixed,
            )?;
            state += 1;
            if parts[0] == "c" {
                t.commit();
            } else {
                drop(t);
                emit(
                    f[0],
                    state,
                    leaders,
                    d.assignment(),
                    d.graph(),
                    &shadow,
                    fixed,
                )?;
                state += 1;
            }
            d.assert_consistent()?;
        }
    }
    Ok(())
}
