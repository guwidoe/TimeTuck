// Adapted from Timefold Community DefaultSolverTest and no_inconsistent_field
// fixtures, Apache-2.0. Exact test/helper attribution: compat/provenance.json.
use timetuck::*;
fn problem(lists: [Vec<usize>; 2], pin: usize) -> ListProblem<usize> {
    ListProblem::new(
        (0..4).collect(),
        vec![
            ListEntity::new(lists[0].clone()).with_pinned_prefix(pin),
            ListEntity::new(lists[1].clone()),
        ],
    )
    .unwrap()
    .with_dependencies(vec![vec![], vec![0], vec![], vec![2]])
    .unwrap()
}
// a1,a2,b1,b2; start/end are minutes after epoch, each duration is 60 minutes.
fn times(lists: &[Vec<usize>]) -> Vec<Option<i64>> {
    let a = ListAssignment::new(lists.to_vec());
    let graph =
        DependencyGraph::new([(0, vec![]), (1, vec![0]), (2, vec![]), (3, vec![2])]).unwrap();
    let dynamic: Vec<_> = lists
        .iter()
        .flat_map(|l| l.windows(2).map(|w| (w[1], w[0])))
        .collect();
    let values = graph
        .evaluate(&dynamic, |&i, deps| {
            a.location(&i)?;
            let mut ready = 0;
            for (_, d) in deps {
                ready = ready.max(d.value?);
            }
            Some(ready + 60)
        })
        .unwrap();
    assert!(
        values.values().all(|v| !v.inconsistent),
        "cyclic states must never reach scoring"
    );
    (0..4).map(|i| values[&i].value).collect()
}
fn score(lists: &[Vec<usize>]) -> HardSoftScore {
    HardSoftScore::new(0, -times(lists).into_iter().flatten().sum::<i64>())
}
fn network() -> ListConstraintNetwork<'static, usize, HardSoftScore> {
    let mut n = ListConstraintNetwork::new(2, 4);
    n.add_constraint(
        "finish early",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        |facts| {
            let lists: Vec<_> = facts
                .iter()
                .map(|f| {
                    let ListFact::Entity(l) = f else { panic!() };
                    l.to_vec()
                })
                .collect();
            score(&lists)
        },
    )
    .unwrap();
    n
}
type Graph = (
    StreamGraph<'static, HardSoftScore>,
    ListStreamSources<usize>,
);
fn graph() -> Result<Graph, GraphError> {
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::new(&mut b);
    let lists = b.group_list(s.entities.port(), |row| ((), row.clone()))?;
    b.score("finish early", &lists, |(_, rows)| {
        let mut lists = vec![vec![]; 2];
        for (i, l) in rows.iter() {
            lists[*i] = l.clone();
        }
        score(&lists)
    })?;
    Ok((b.build(), s))
}
type RunResult = Result<(SolverResult<ListAssignment<usize>, HardSoftScore>, usize), String>;
fn run(p: &ListProblem<usize>, stream: bool, custom: Option<bool>) -> RunResult {
    let plan = if custom.is_some() {
        SolverSequence {
            phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
            ..Default::default()
        }
    } else {
        SolverConfig {
            local_search: vec![LocalSearchConfig {
                max_steps: 10,
                assert_scores: true,
                ..Default::default()
            }],
            ..Default::default()
        }
        .into()
    };
    let mut best = 0;
    let mut events = |e: SolverEvent<'_, ListAssignment<usize>, HardSoftScore>| {
        if matches!(e, SolverEvent::Best(_)) {
            best += 1;
        }
    };
    let movement = ListMove::Composite(vec![
        ListMove::Assign {
            value: 3,
            destination: ListPosition {
                entity: 0,
                index: 0,
            },
        },
        ListMove::Assign {
            value: 2,
            destination: ListPosition {
                entity: 0,
                index: 1,
            },
        },
    ]);
    let r = if stream {
        solve_list_stream_with_commands_and_events(
            p,
            graph,
            plan,
            |_, ctx| {
                if custom == Some(true) {
                    ctx.execute(&movement)?;
                }
                Ok(())
            },
            &mut events,
        )
        .map_err(|e| format!("{e:?}"))
    } else {
        solve_list_with_commands_and_events(
            p,
            &network(),
            plan,
            |_, ctx| {
                if custom == Some(true) {
                    ctx.execute(&movement)?;
                }
                Ok(())
            },
            &mut events,
        )
        .map_err(|e| format!("{e:?}"))
    }?;
    Ok((r, best))
}
#[test]
fn solve_when_ignoring_inconsistent_solutions_unassigns_if_initial_solution_inconsistent() {
    for stream in [false, true] {
        let p = problem([vec![3, 2], vec![0, 1]], 0);
        let (r, best) = run(&p, stream, Some(false)).unwrap();
        assert_eq!(best, 0);
        assert!(r.assignment.lists()[0].is_empty());
        assert_eq!(r.assignment.lists()[1].len(), 2);
        let a1 = r.assignment.element(&0).unwrap();
        let a2 = r.assignment.element(&1).unwrap();
        assert_eq!(a1.position.entity, 1);
        assert_eq!(a1.previous, None);
        assert_eq!(a2.position.entity, 1);
        assert_eq!(a2.previous, Some(&0));
        assert!(r.assignment.element(&2).is_none());
        assert!(r.assignment.element(&3).is_none());
        assert_eq!(p.initial_assignment().lists(), &[vec![3, 2], vec![0, 1]]);
    }
}
#[test]
fn solve_when_ignoring_inconsistent_solutions_throws_if_inconsistent_entity_pinned() {
    for stream in [false, true] {
        let p = problem([vec![3, 2], vec![1, 0]], 1);
        let error = run(&p, stream, None).unwrap_err();
        assert!(
            error.contains("InitialScoring") && error.contains("PinnedDependencyCycle"),
            "{error}"
        );
        assert!(
            error.contains("value: 3") && error.contains("entity: 0, index: 0"),
            "{error}"
        );
    }
}
#[test]
fn solve_custom_phase_returns_structurally_flawed() {
    for stream in [false, true] {
        let error = run(&problem([vec![3, 2], vec![0, 1]], 0), stream, Some(true)).unwrap_err();
        assert!(
            error.contains("Custom") && error.contains("DependencyCycle { values: [2, 3] }"),
            "{error}"
        );
    }
}
#[test]
fn solve_ignore_inconsistent() {
    for stream in [false, true] {
        let (r, best) = run(&problem([vec![], vec![]], 0), stream, None).unwrap();
        assert!(best > 0);
        assert!(r.complete);
        assert_eq!(r.score, Some(HardSoftScore::new(0, -360)));
        let end = times(r.assignment.lists());
        assert!(end[1].unwrap() - 60 >= end[0].unwrap());
        assert!(end[3].unwrap() - 60 >= end[2].unwrap());
    }
}
