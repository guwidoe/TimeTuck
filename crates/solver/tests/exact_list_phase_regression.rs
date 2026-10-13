use timetuck::*;
#[test]
fn varying_native_models_execute_and_restore_real_moves() {
    for count in 1..=4 {
        let p = ListProblem::new(
            (0..count).collect::<Vec<i32>>(),
            vec![ListEntity::new(vec![]); 2],
        )
        .unwrap()
        .allowing_unassigned();
        let score = |a: &ListAssignment<i32>| {
            SimpleScore::new(-(a.lists().iter().map(|l| l.len() * l.len()).sum::<usize>() as i64))
                .with_structural(
                    -((count as usize - a.lists().iter().map(Vec::len).sum::<usize>()) as i64),
                )
        };
        for order in [
            NodeExploration::DepthFirst,
            NodeExploration::BreadthFirst,
            NodeExploration::ScoreFirst,
            NodeExploration::OptimisticBoundFirst,
            NodeExploration::InputOrderDepthFirst,
        ] {
            let mut model = ExactListPhaseModel::new(
                &p,
                vec![1, 0],
                (0..count as usize).rev().collect(),
                score,
                |_, _, s: &SimpleScore| Some(s.with_structural(0)),
            )
            .unwrap();
            let mut observed = 0;
            let result = run_exact_phase(
                &mut model,
                ExactMovePhaseConfig {
                    kind: ExactPhaseKind::BranchAndBound,
                    exploration: Some(order),
                    max_steps: 10000,
                },
                |a, s| {
                    p.validate(a).unwrap();
                    assert_eq!(*s, score(a));
                    observed += 1;
                },
            )
            .unwrap();
            assert_eq!(observed, result.steps);
            assert!(p.is_complete(&result.best.assignment));
            let x = count as i64 / 2;
            let y = count as i64 - x;
            assert_eq!(result.best.score, SimpleScore::new(-(x * x + y * y)));
            assert_eq!(model.initial().unwrap(), p.initial_assignment());
        }
    }
}
#[test]
fn explicit_orders_ranges_and_pins_are_validated() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1]).with_pinned(true),
            ListEntity::new(vec![]).with_value_range(vec![2]),
            ListEntity::new(vec![]).with_value_range(vec![3]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let s = |a: &ListAssignment<i32>| {
        SimpleScore::ZERO
            .with_structural(-((3 - a.lists().iter().map(Vec::len).sum::<usize>()) as i64))
    };
    let mut model = ExactListPhaseModel::new(
        &p,
        vec![2, 1, 0],
        vec![2, 0, 1],
        s,
        |_, _, _: &SimpleScore| None,
    )
    .unwrap();
    let initial = model.initial().unwrap();
    let moves = model.moves(0, &initial).unwrap();
    assert_eq!(moves.len(), 2);
    let r = run_exact_phase(
        &mut model,
        ExactMovePhaseConfig {
            kind: ExactPhaseKind::BruteForce,
            exploration: None,
            max_steps: 100,
        },
        |_, _| {},
    )
    .unwrap();
    assert_eq!(r.best.assignment.lists(), [vec![1], vec![2], vec![3]]);
    assert!(matches!(
        ExactListPhaseModel::new(
            &p,
            vec![0, 0, 2],
            vec![0, 1, 2],
            s,
            |_, _, _: &SimpleScore| None
        ),
        Err(ExactListPhaseError::InvalidEntityOrder)
    ));
    assert!(matches!(
        ExactListPhaseModel::new(&p, vec![0, 1, 2], vec![0, 1], s, |_, _, _: &SimpleScore| {
            None
        }),
        Err(ExactListPhaseError::InvalidValueOrder)
    ));
    let capped = run_exact_phase(
        &mut model,
        ExactMovePhaseConfig {
            max_steps: 0,
            ..Default::default()
        },
        |_, _| panic!("no steps"),
    )
    .unwrap();
    assert_eq!(capped.generated, 0);
    assert_eq!(capped.steps, 0);
    assert_eq!(capped.working, initial);
}
