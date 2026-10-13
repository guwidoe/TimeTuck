//! Original varied domains/pins, optimality, budget and order-error checks.
use timetuck::*;
#[test]
fn real_varied_domains_and_pins_match_independent_cartesian_optimum() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![7, 11]),
        Variable::new(vec![20, 30]),
        Variable::new(vec![9]).with_initial(9).with_pinned(true),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("objective", vec![0, 1, 2], |v| {
        SimpleScore::new(
            v[0].copied().map_or(0, i64::from) * 3 - v[1].copied().map_or(0, i64::from)
                + v[2].copied().map_or(0, i64::from),
        )
    })
    .unwrap();
    let oracle = (p.variables()[0].domain().iter().flat_map(|&x| {
        p.variables()[1]
            .domain()
            .iter()
            .map(move |&y| 3 * i64::from(x) - i64::from(y) + 9)
    }))
    .max()
    .unwrap();
    for order in [
        NodeExploration::DepthFirst,
        NodeExploration::BreadthFirst,
        NodeExploration::ScoreFirst,
        NodeExploration::OptimisticBoundFirst,
        NodeExploration::InputOrderDepthFirst,
    ] {
        let mut model = ExactBasicPhaseModel::new(
            &p,
            &n,
            vec![1, 0, 2],
            vec![vec![1, 0], vec![0, 1], vec![0]],
            |_: &Assignment<i32>, _: &SimpleScore| None::<SimpleScore>,
        )
        .unwrap();
        let mut observed = 0;
        let result = run_exact_phase(
            &mut model,
            ExactMovePhaseConfig {
                kind: ExactPhaseKind::BranchAndBound,
                exploration: Some(order),
                max_steps: 100,
            },
            |a, s| {
                assert_eq!(a.values()[2], Some(9));
                assert_eq!(*s, n.calculate(&p, a).unwrap());
                observed += 1;
            },
        )
        .unwrap();
        assert_eq!(result.steps, observed);
        assert_eq!(result.best.score, SimpleScore::new(oracle));
        assert_eq!(
            result.best.assignment.values(),
            &[Some(11), Some(20), Some(9)]
        );
        assert_eq!(p.initial_assignment().values(), &[None, None, Some(9)]);
    }
}
#[test]
fn bounded_expansion_keeps_first_best_witness_and_zero_budget_has_no_moves() {
    let p = FiniteProblem::new(vec![Variable::new(vec![4, 5]); 2]).unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("distinct", vec![0, 1], |v| {
        SimpleScore::new(-i64::from(v[0].is_some() && v[0] == v[1]))
    })
    .unwrap();
    let mut model = ExactBasicPhaseModel::new(
        &p,
        &n,
        vec![0, 1],
        vec![vec![0, 1]; 2],
        |_: &Assignment<i32>, s: &SimpleScore| Some((*s).with_structural(0)),
    )
    .unwrap();
    let zero = run_exact_phase(
        &mut model,
        ExactMovePhaseConfig {
            max_steps: 0,
            ..Default::default()
        },
        |_, _| panic!("zero budget observer"),
    )
    .unwrap();
    assert_eq!(zero.generated, 0);
    assert_eq!(zero.steps, 0);
    assert_eq!(zero.best.assignment, p.initial_assignment());
    let full = run_exact_phase(&mut model, ExactMovePhaseConfig::default(), |_, _| {}).unwrap();
    assert_eq!(full.best.assignment.values(), &[Some(4), Some(5)]);
    assert_eq!(full.best.score, SimpleScore::new(0));
    assert_eq!(full.steps, 2);
    assert_eq!(full.generated, 6);
}
#[test]
fn invalid_permutations_reject_and_initialized_problem_has_no_steps() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1]).with_initial(1)]).unwrap();
    let n = ConstraintNetwork::<i32, SimpleScore>::new(1);
    assert!(matches!(
        ExactBasicPhaseModel::new(
            &p,
            &n,
            vec![0, 0],
            vec![vec![0]],
            |_: &Assignment<i32>, _: &SimpleScore| None::<SimpleScore>
        ),
        Err(ExactBasicModelError::EntityOrder)
    ));
    assert!(matches!(
        ExactBasicPhaseModel::new(
            &p,
            &n,
            vec![0],
            vec![vec![1]],
            |_: &Assignment<i32>, _: &SimpleScore| None::<SimpleScore>
        ),
        Err(ExactBasicModelError::ValueOrder { variable: 0 })
    ));
    let mut model = ExactBasicPhaseModel::new(
        &p,
        &n,
        vec![0],
        vec![vec![0]],
        |_: &Assignment<i32>, _: &SimpleScore| None::<SimpleScore>,
    )
    .unwrap();
    let result = run_exact_phase(&mut model, ExactMovePhaseConfig::default(), |_, _| {
        panic!("already initialized")
    })
    .unwrap();
    assert_eq!(result.steps, 0);
    assert_eq!(result.generated, 0);
    assert_eq!(result.best.assignment, p.initial_assignment());
}
