//! Original example: shared graph constraints used by construction and local search.
use timetuck::*;
const N: usize = 24;
const COLORS: usize = 4;
fn graph() -> Result<
    (
        StreamGraph<'static, HardSoftScore>,
        PlanningStreamSource<usize>,
    ),
    GraphError,
> {
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<usize>)>();
    let present = b.filter(source.port(), |(_, v)| v.is_some())?;
    let assigned = b.map(&present, |&(id, v)| (id, v.unwrap()))?;
    let pairs = b.join_equal(&assigned, &assigned, |row| row.1, |row| row.1)?;
    let neighbors = b.filter(&pairs, |(a, b)| a.0 + 1 == b.0)?;
    b.score("adjacent same color", &neighbors, |_| {
        HardSoftScore::new(-1, 0)
    })?;
    let loads = b.group_collect(&assigned, |&(_, color)| (color, 1))?;
    b.score("color capacity", &loads, |(_, s)| {
        HardSoftScore::new(-(s.sum - (N / COLORS) as i64).max(0), 0)
    })?;
    b.score("preferred color", &assigned, |&(id, color)| {
        HardSoftScore::new(0, -i64::from(color != id % COLORS))
    })?;
    Ok((b.build(), source))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let problem = FiniteProblem::new(vec![Variable::new((0..COLORS).collect()); N])?;
    let (graph, source) = graph()?;
    let construction =
        greedy_stream_construction(&problem, graph, source, ConstructionConfig::default())?;
    let initial = construction.solution().expect("construction completed");
    let (graph, source) = self::graph()?;
    let improvement = stream_local_search_from(
        &problem,
        graph,
        source,
        initial.assignment,
        LocalSearchConfig::default(),
    )?;
    let best = improvement.best.expect("initial score exists");
    // Independent verification of the entire returned assignment and score.
    problem.validate(&best.assignment)?;
    assert!(problem.is_complete(&best.assignment));
    let values: Vec<_> = best
        .assignment
        .values()
        .iter()
        .map(|v| v.unwrap())
        .collect();
    let conflicts = values.windows(2).filter(|pair| pair[0] == pair[1]).count();
    let overload: usize = (0..COLORS)
        .map(|c| {
            values
                .iter()
                .filter(|&&v| v == c)
                .count()
                .saturating_sub(N / COLORS)
        })
        .sum();
    let preferences = values
        .iter()
        .enumerate()
        .filter(|&(id, v)| *v != id % COLORS)
        .count();
    let checked = HardSoftScore::new(-((conflicts + overload) as i64), -(preferences as i64));
    assert_eq!(best.score, checked);
    assert_eq!(best.score, HardSoftScore::ZERO); // All contributions are nonpositive.
    println!("{N} variables / {COLORS} colors: {}", best.score);
    println!(
        "{} score evaluations, {} terminal callbacks (including both phase initializations)",
        construction.evaluations + improvement.evaluations,
        construction.constraint_evaluations + improvement.constraint_evaluations
    );
    println!("Persistent snapshots; this example is not a performance comparison.");
    Ok(())
}
