// Adapted from Timefold Community DefaultSolverTest and shadow fixtures,
// Apache-2.0. See compat/provenance.json for exact attribution.
#[path = "../examples/concurrent_support/mod.rs"]
mod support;
use timetuck::*;
fn inverse_score(values: impl IntoIterator<Item = Option<usize>>) -> SimpleScore {
    let mut inverse = [Vec::new(), Vec::new()];
    let mut unassigned = 0;
    for (entity, value) in values.into_iter().enumerate() {
        match value {
            Some(v) => inverse[v].push(entity),
            None => unassigned += 1,
        }
    }
    SimpleScore::new(
        -inverse
            .iter()
            .map(|v| (v.len() * v.len()) as i64)
            .sum::<i64>()
            - 100 * unassigned,
    )
}
#[test]
fn solve_stale_builtin_shadows() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0usize, 1])
            .allowing_unassigned()
            .with_initial(0);
        2
    ])
    .unwrap();
    for stream in [false, true] {
        let mut events = vec![];
        let observer = |e: SolverEvent<'_, Assignment<usize>, SimpleScore>| {
            if let SolverEvent::Best(u) = e {
                events.push(u.score);
            }
        };
        let plan = SolverConfig {
            local_search: vec![LocalSearchConfig {
                max_steps: 10,
                assert_scores: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        let r = if stream {
            solve_stream_with_events(
                &p,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let source = b.source::<usize, (usize, Option<usize>)>();
                    let rows = b.collect_list(source.port(), |row| *row)?;
                    b.score("inverse balance and unassigned", &rows, |rows| {
                        inverse_score(rows.iter().map(|(_, v)| *v))
                    })?;
                    Ok((b.build(), source))
                },
                plan,
                observer,
            )
            .unwrap()
        } else {
            let mut n = ConstraintNetwork::new(2);
            n.add_constraint("inverse balance and unassigned", vec![0, 1], |v| {
                inverse_score(v.iter().map(|v| v.copied()))
            })
            .unwrap();
            solve_with_events(&p, &n, plan, observer).unwrap()
        };
        // Initial score is rebuilt from genuine assignments, not empty inverse shadows.
        assert_eq!(events.first(), Some(&SimpleScore::new(-4)));
        assert_eq!(r.score, Some(SimpleScore::new(-2)));
        let mut values = r.assignment.values().to_vec();
        values.sort();
        assert_eq!(values, vec![Some(0), Some(1)]);
    }
}
#[test]
fn solve_stale_declarative_shadows() {
    let schedule = support::Schedule::new(&[vec![0, 1], vec![2, 3]], 4);
    let p = ListProblem::new(
        (0..4).collect(),
        vec![ListEntity::new(vec![0, 2]), ListEntity::new(vec![3, 1])],
    )
    .unwrap();
    for stream in [false, true] {
        let mut events = vec![];
        let observer = |e: SolverEvent<'_, ListAssignment<usize>, HardSoftScore>| {
            if let SolverEvent::Best(u) = e {
                events.push(u.score);
            }
        };
        let plan = SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: u64::MAX,
                max_time: Some(std::time::Duration::from_secs(60)),
                stop_when_feasible: true,
                assert_scores: true,
                ..Default::default()
            })],
            ..Default::default()
        };
        let r = if stream {
            solve_list_stream_with_events(
                &p,
                || {
                    let (g, s, _) = schedule.graph(2);
                    Ok((g, s))
                },
                plan,
                observer,
            )
            .unwrap()
        } else {
            let mut n = ListConstraintNetwork::new(2, 4);
            n.add_constraint(
                "concurrent schedule",
                vec![ListScope::Entity(0), ListScope::Entity(1)],
                |facts| {
                    let lists: Vec<_> = facts
                        .iter()
                        .map(|f| {
                            let ListFact::Entity(l) = f else { panic!() };
                            l.to_vec()
                        })
                        .collect();
                    schedule.score(&lists)
                },
            )
            .unwrap();
            solve_list_with_events(&p, &n, plan, observer).unwrap()
        };
        assert_eq!(events.first(), Some(&HardSoftScore::new(-4, 0)));
        assert_eq!(r.assignment.lists(), &[vec![2, 0], vec![3, 1]]);
        assert_eq!(r.score, Some(HardSoftScore::new(0, -240)));
        assert!(r.complete);
    }
}
