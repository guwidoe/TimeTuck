// Adapted from pinned Timefold Community DefaultConstructionHeuristicPhaseTest.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
use timetuck::*;
fn plan() -> SolverPlan<'static> {
    SolverPlan {
        construction: ConstructionConfig {
            optional: OptionalConstruction::NonWorsening,
            ..Default::default()
        },
        phases: vec![],
        ..Default::default()
    }
}
#[test]
fn solve_with_allows_unassigned_basic_variable() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2])
            .allowing_unassigned()
            .with_initial(1),
        Variable::new(vec![1, 2]).allowing_unassigned(),
        Variable::new(vec![1, 2]).allowing_unassigned(),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("optional assignment", vec![0, 1, 2], |v| {
        let mut score = 0;
        for (i, left) in v.iter().enumerate() {
            if left.is_none() {
                score -= 1;
            } else {
                for (j, right) in v.iter().enumerate() {
                    if i != j && left == right {
                        score -= 1000;
                    }
                }
            }
        }
        SimpleScore::new(score)
    })
    .unwrap();
    let r = solve(&p, &n, plan()).unwrap();
    assert_eq!(r.score, Some(SimpleScore::new(-1)));
    assert_eq!(r.assignment.values()[0], Some(1));
    assert_eq!(r.assignment.values()[1], Some(2));
    assert_eq!(r.assignment.values()[2], None);
    assert!(r.complete);
}
#[test]
fn solve_with_allows_unassigned_values_list_variable() {
    let p = ListProblem::new(vec![1, 2, 3, 4], vec![ListEntity::new(vec![1, 2])])
        .unwrap()
        .allowing_unassigned();
    let mut n = ListConstraintNetwork::new(1, 4);
    n.add_constraint("list size", vec![ListScope::Entity(0)], |v| {
        let ListFact::Entity(list) = &v[0] else {
            panic!()
        };
        SimpleScore::new(-(list.len() as i64))
    })
    .unwrap();
    let r = solve_list(&p, &n, plan()).unwrap();
    assert_eq!(r.score, Some(SimpleScore::new(-2)));
    assert_eq!(r.assignment.lists()[0], vec![1, 2]);
    assert!(r.complete);
}
