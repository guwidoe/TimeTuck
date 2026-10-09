//! Original stream constraints equivalent to the solver comparison fixture.
use crate::solver_support::Case;
use std::error::Error;
use timetuck::*;
type Job = (usize, usize, i64);
type ConflictId = ((usize, usize), usize);
pub struct Network<'a> {
    pub graph: StreamGraph<'a, HardSoftScore>,
    pub source: PlanningStreamSource<usize>,
    pub conflicts: StreamConstraint<ConflictId, usize>,
    pub preferences: StreamConstraint<usize, (usize, IntSummary)>,
}
pub fn build(case: &Case) -> Result<Network<'_>, Box<dyn Error>> {
    build_with_batching(case, false)
}
pub fn build_with_batching(case: &Case, batched: bool) -> Result<Network<'_>, Box<dyn Error>> {
    if case.weights.len() > 10000
        || case
            .weights
            .iter()
            .any(|row| row.len() > 128 || row.iter().any(|w| !(-1000..=1000).contains(w)))
    {
        return Err("adapter size/weight bound".into());
    }
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<usize>)>();
    let assigned = b.filter(source.port(), |(_, v)| v.is_some())?;
    let jobs = b.map(&assigned, |&(id, v)| {
        let value = v.unwrap();
        (id, value, case.weights[id][value])
    })?;
    let (identities, edges) = if batched {
        let linked = b.join_fixed_links_batched(
            &jobs,
            &jobs,
            case.edges.iter().enumerate().map(|(e, &(i, j))| (e, i, j)),
            |_, a, b| a.1 == b.1,
            |&e, _, _| e,
        )?;
        (linked, None)
    } else {
        let edges = b.source::<usize, (usize, usize, usize)>();
        let a = b.join_equal(edges.port(), &jobs, |e| e.1, |j: &Job| j.0)?;
        let ab = b.join_equal(&a, &jobs, |(e, _)| e.2, |j: &Job| j.0)?;
        let conflicts = b.filter(&ab, |((_, a), b)| a.1 == b.1)?;
        (b.map(&conflicts, |((e, _), _)| e.0)?, Some(edges))
    };
    let conflicts = if batched {
        b.score_fixed(
            "conflicts",
            &identities,
            case.edges
                .iter()
                .enumerate()
                .map(|(e, &(i, j))| ((e, i), j)),
            |_| HardSoftScore::new(-1, 0),
        )?
    } else {
        b.score("conflicts", &identities, |_| HardSoftScore::new(-1, 0))?
    };
    let grouped = if batched {
        b.group_collect_batched(&jobs, |&(_, v, w)| (v, w))?
    } else {
        b.group_collect(&jobs, |&(_, v, w)| (v, w))?
    };
    let nonzero = b.filter(&grouped, |(_, s)| s.sum != 0)?;
    let preferences = b.score("preferences", &nonzero, |(_, s)| {
        HardSoftScore::new(0, impact(s.sum))
    })?;
    let mut graph = b.build();
    if let Some(edges) = edges {
        let mut tx = graph.transaction();
        for (id, &(a, b)) in case.edges.iter().enumerate() {
            tx.apply(&edges, TupleChange::put(id, (id, a, b)))?;
        }
        tx.commit()?;
    }
    Ok(Network {
        graph,
        source,
        conflicts,
        preferences,
    })
}
pub fn emit(
    id: &str,
    state: usize,
    graph: &StreamGraph<'_, HardSoftScore>,
    assignment: &Assignment<usize>,
    conflicts: &StreamConstraint<ConflictId, usize>,
    preferences: &StreamConstraint<usize, (usize, IntSummary)>,
) -> Result<(), Box<dyn Error>> {
    let mut edges = Vec::new();
    for (_, &(edge, s)) in graph.constraint_matches(conflicts)? {
        if s != HardSoftScore::new(-1, 0) {
            return Err("invalid conflict contribution".into());
        }
        edges.push(edge);
    }
    edges.sort_unstable();
    let mut groups = Vec::new();
    for (&key, ((_, summary), s)) in graph.constraint_matches(preferences)? {
        if *s != HardSoftScore::new(0, impact(summary.sum)) {
            return Err("invalid group contribution".into());
        }
        groups.push(format!("{key}:{}", summary.sum));
    }
    let score = graph.score();
    println!(
        "{id}\t{state}\t{}\t{}\t{}\t{}\t{}",
        u8::from(assignment.values().iter().all(Option::is_some)),
        score.hard(),
        score.soft(),
        edges
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(","),
        groups.join(",")
    );
    Ok(())
}

pub fn impact(sum: i64) -> i64 {
    sum * sum.abs()
}
pub fn score(case: &Case, assignment: &Assignment<usize>) -> HardSoftScore {
    let hard = case.score(assignment).hard();
    let mut groups = std::collections::BTreeMap::<usize, i64>::new();
    for (id, value) in assignment.values().iter().enumerate() {
        if let Some(value) = value {
            *groups.entry(*value).or_default() += case.weights[id][*value];
        }
    }
    HardSoftScore::new(hard, groups.values().copied().map(impact).sum())
}
