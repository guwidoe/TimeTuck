//! Original coupled route-search adapter; no upstream method credit or timing claim.
use std::{
    error::Error,
    io::{self, BufRead},
};
use timetuck::*;

fn numbers<T: std::str::FromStr>(s: &str) -> Result<Vec<T>, T::Err> {
    s.split(',').map(str::parse).collect()
}
fn route_score(depot: i64, route: &[usize], points: &[i64], capacity: usize) -> HardSoftScore {
    let mut previous = depot;
    let mut distance = 0;
    for &customer in route {
        distance += (points[customer] - previous).abs();
        previous = points[customer];
    }
    distance += (previous - depot).abs();
    HardSoftScore::new(-(route.len().saturating_sub(capacity) as i64), -distance)
}
fn score(a: &MixedAssignment<i64, usize>, points: &[i64], capacities: &[usize]) -> HardSoftScore {
    a.lists()
        .lists()
        .iter()
        .enumerate()
        .fold(HardSoftScore::new(0, 0), |s, (i, route)| {
            s + route_score(a.basic().values()[i].unwrap(), route, points, capacities[i])
        })
}
fn emit(
    id: &str,
    mode: &str,
    best: ScoredMixedAssignment<i64, usize, HardSoftScore>,
    counts: (u64, u64),
    reason: impl std::fmt::Debug,
) {
    let depots = best
        .assignment
        .basic()
        .values()
        .iter()
        .map(|v| v.unwrap().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let routes = best
        .assignment
        .lists()
        .lists()
        .iter()
        .map(|r| {
            if r.is_empty() {
                "-".to_owned()
            } else {
                r.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            }
        })
        .collect::<Vec<_>>()
        .join(";");
    println!(
        "{id}\t{mode}\t{}\t{}\t{depots}\t{routes}\t{}\t{}\t{reason:?}",
        best.score.hard(),
        best.score.soft(),
        counts.0,
        counts.1
    );
}
fn main() -> Result<(), Box<dyn Error>> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        let f: Vec<_> = line.split('\t').collect();
        if f.len() != 4 {
            return Err("four fields required".into());
        }
        let depots = numbers::<i64>(f[1])?;
        let points = numbers::<i64>(f[2])?;
        let capacities = numbers::<usize>(f[3])?;
        if depots.len() > 3
            || points.len() > 4
            || capacities.len() > 3
            || depots
                .iter()
                .chain(&points)
                .any(|v| !(-1000..=1000).contains(v))
        {
            return Err("small comparison bounds exceeded".into());
        }
        let owners = capacities.len();
        let p = MixedProblem::new(
            FiniteProblem::new((0..owners).map(|_| Variable::new(depots.clone())).collect())?,
            ListProblem::new(
                (0..points.len()).collect(),
                (0..owners).map(|_| ListEntity::new(vec![])).collect(),
            )?,
        );
        let exact = exhaustive_mixed_search(&p, |a| score(a, &points, &capacities), 1_000_000);
        if !exact.is_optimal() {
            return Err("exact comparison budget exhausted".into());
        }
        emit(
            f[0],
            "exact",
            exact.best.unwrap(),
            (exact.evaluations, 0),
            exact.termination,
        );
        let initial = MixedAssignment::new(
            Assignment::new(vec![Some(depots[0]); owners]),
            ListAssignment::new(
                (0..owners)
                    .map(|i| {
                        if i == 0 {
                            (0..points.len()).collect()
                        } else {
                            vec![]
                        }
                    })
                    .collect(),
            ),
        );
        let config = MixedLocalSearchConfig {
            search: LocalSearchConfig {
                max_steps: 20,
                max_evaluations: 1_000_000,
                selection: MoveSelection::BestAccepted,
                assert_scores: true,
                ..Default::default()
            },
            joint_moves: true,
        };
        let mut n = MixedConstraintNetwork::new(owners, owners, points.len());
        for (i, &capacity) in capacities.iter().enumerate() {
            let points = &points;
            n.add_constraint(
                format!("route {i}"),
                vec![MixedScope::Basic(i), MixedScope::ListEntity(i)],
                move |facts| {
                    let (MixedFact::Basic(depot), MixedFact::ListEntity(route)) =
                        (&facts[0], &facts[1])
                    else {
                        unreachable!()
                    };
                    route_score(*depot.unwrap(), route, points, capacity)
                },
            )?;
        }
        let result = mixed_local_search_from(&p, &n, initial.clone(), config)?;
        emit(
            f[0],
            "scoped",
            result.best.unwrap(),
            (result.evaluations, result.completed_steps),
            result.termination,
        );
        let mut b = StreamGraphBuilder::new();
        let sources = MixedStreamSources::new(&mut b);
        let joined = b.join_equal(
            sources.basic.port(),
            sources.lists.entities.port(),
            |(i, _)| *i,
            |(i, _)| *i,
        )?;
        b.score("routes", &joined, |((i, depot), (_, route))| {
            route_score(depot.unwrap(), route, &points, capacities[*i])
        })?;
        let result = mixed_stream_local_search_from(&p, b.build(), sources, initial, config)?;
        emit(
            f[0],
            "stream",
            result.best.unwrap(),
            (result.evaluations, result.completed_steps),
            result.termination,
        );
    }
    Ok(())
}
