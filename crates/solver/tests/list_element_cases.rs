// Adapted from Community ListElementShadowVariableTest, Apache-2.0.
// Fixture provenance and modifications: compat/provenance.json.
#[path = "../examples/list_element_support/mod.rs"]
mod support;
use timetuck::*;
fn problem(n: usize, lists: Vec<Vec<usize>>) -> ListProblem<usize> {
    ListProblem::new(
        (0..n).collect(),
        lists.into_iter().map(ListEntity::new).collect(),
    )
    .unwrap()
    .allowing_unassigned()
}
fn ends(g: &StreamGraph<'_, SimpleScore>, c: &support::Ends) -> Vec<i32> {
    g.constraint_matches(c)
        .unwrap()
        .values()
        .map(|(v, _)| v.1)
        .collect()
}
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
#[test]
fn update_shadow_variables() {
    let p = problem(4, vec![vec![0, 1], vec![2, 3]]);
    let (g, s, c, v) = support::ordered(vec![0, 2], vec![1; 4]);
    let d = ListStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    assert_eq!(
        d.graph()
            .constraint_matches(&v)
            .unwrap()
            .values()
            .map(|(v, _)| v.end)
            .collect::<Vec<_>>(),
        vec![Some(1), Some(2), Some(3), Some(4)]
    );
    assert_eq!(ends(d.graph(), &c), vec![2, 4]);
}
#[test]
fn update_shadow_variables_empty_list() {
    let p = problem(1, vec![vec![]]);
    let (g, s, c, v) = support::ordered(vec![3], vec![1]);
    let d = ListStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    assert_eq!(ends(d.graph(), &c), vec![3]);
    let values = d.graph().constraint_matches(&v).unwrap();
    assert_eq!(values[&0].0.owner, None);
    assert_eq!(values[&0].0.end, None);
}
#[test]
fn list_moves_update_aggregates() {
    let p = problem(3, vec![vec![0, 1, 2], vec![]]);
    let (g, s, c, _) = support::ordered(vec![0, 10], vec![1; 3]);
    let mut d = ListStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    assert_eq!(ends(d.graph(), &c), vec![3, 10]);
    d.apply(&ListMove::Composite(vec![
        ListMove::Change {
            source: pos(0, 2),
            destination: pos(1, 0),
        },
        ListMove::Swap {
            left: pos(0, 0),
            right: pos(0, 1),
        },
    ]))
    .unwrap()
    .commit();
    assert_eq!(ends(d.graph(), &c), vec![2, 11]);
    d.assert_consistent().unwrap();
}
#[test]
fn basic_variable_change_retriggers_aggregate_without_list_change() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new((1..=9).collect())
                .allowing_unassigned()
                .with_initial(2),
            Variable::new((1..=9).collect())
                .allowing_unassigned()
                .with_initial(3),
        ])
        .unwrap(),
        ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])]).unwrap(),
    );
    let (g, s, c, v) = support::mixed();
    let mut d = MixedStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    assert_eq!(d.graph().constraint_matches(&c).unwrap()[&0].0.1, Some(7));
    d.apply(&MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(5),
        },
        lists: ListMove::Composite(vec![]),
    })
    .unwrap()
    .commit();
    assert_eq!(d.graph().constraint_matches(&v).unwrap()[&0].0.1, Some(6));
    assert_eq!(d.graph().constraint_matches(&c).unwrap()[&0].0.1, Some(10));
    d.assert_consistent().unwrap();
}
fn solve_case(lists: Vec<Vec<usize>>, n: usize) {
    let starts = (0..lists.len()).map(|i| i as i32).collect::<Vec<_>>();
    let durations = (0..n).map(|i| 1 + (i % 3) as i32).collect::<Vec<_>>();
    let p = problem(n, lists);
    let result = solve_list_stream(
        &p,
        || {
            let (g, s, _, _) = support::ordered(starts.clone(), durations.clone());
            Ok((g, s))
        },
        SolverConfig {
            limits: SolverLimits {
                max_move_evaluations: Some(1000),
                ..Default::default()
            },
            construction: ConstructionConfig {
                assert_scores: true,
                ..Default::default()
            },
            local_search: vec![LocalSearchConfig {
                assert_scores: true,
                ..Default::default()
            }],
        },
    )
    .unwrap();
    assert!(result.work.move_evaluations <= 1000);
    let (g, s, c, v) = support::ordered(starts.clone(), durations);
    let d = ListStreamScoreDirector::new(&p, g, s, result.assignment.clone()).unwrap();
    assert_eq!(Some(d.score()), result.score);
    let observed = ends(d.graph(), &c);
    let times = d.graph().constraint_matches(&v).unwrap();
    for (e, list) in result.assignment.lists().iter().enumerate() {
        let expected = list.last().map_or(starts[e], |v| times[v].0.end.unwrap());
        assert_eq!(observed[e], expected);
    }
}
#[test]
fn solve_with_full_assert_from_initialized_solution() {
    solve_case(vec![vec![0, 1, 2], vec![3]], 4);
}
#[test]
fn solve_with_full_assert() {
    solve_case(vec![vec![]; 3], 6);
}
