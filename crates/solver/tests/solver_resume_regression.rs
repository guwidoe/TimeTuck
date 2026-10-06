//! Original resume and partial-pin regressions; no upstream method credit.
use timetuck::*;

#[test]
fn resume_preserves_metadata_and_rejects_changed_pins_before_rebasing() {
    let basic = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
        Variable::new(vec![1, 2]).allowing_unassigned(),
    ])
    .unwrap()
    .with_pillar_schema(PillarSchema::new(vec![vec![0], vec![1]]).unwrap())
    .unwrap();
    let lists = ListProblem::new(
        vec![0, 1, 2],
        vec![
            ListEntity::new(vec![0])
                .with_pinned_prefix(1)
                .with_value_range(vec![0, 1]),
            ListEntity::new(vec![2]).with_pinned(true),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let model = MixedProblem::new(basic, lists);
    let original = model.initial_assignment();
    let next = MixedAssignment::new(
        Assignment::new(vec![Some(0), Some(2)]),
        ListAssignment::new(vec![vec![0, 1], vec![2]]),
    );
    let resumed = model.with_initial_assignment(&next).unwrap();
    assert_eq!(resumed.initial_assignment(), next);
    assert_eq!(model.initial_assignment(), original);
    assert_eq!(
        resumed.basic().pillar_schema(),
        model.basic().pillar_schema()
    );
    assert_eq!(resumed.basic().variables()[1].domain(), &[1, 2]);
    assert!(resumed.basic().variables()[1].allows_unassigned());
    assert_eq!(resumed.lists().values(), &[0, 1, 2]);
    assert_eq!(
        resumed.lists().entities()[0].value_range(),
        Some([0, 1].as_slice())
    );
    assert_eq!(resumed.lists().entities()[0].pinned_prefix(), 1);
    assert!(resumed.lists().entities()[1].is_pinned());
    assert!(resumed.lists().allows_unassigned());
    // A full restart may deliberately clear movable assignments again.
    assert_eq!(
        resumed
            .with_initial_assignment(&original)
            .unwrap()
            .initial_assignment(),
        original
    );
    for values in [
        vec![Some(1), Some(2)],
        vec![Some(0), Some(9)],
        vec![Some(0)],
    ] {
        assert!(
            model
                .with_initial_assignment(&MixedAssignment::new(
                    Assignment::new(values),
                    next.lists().clone()
                ))
                .is_err()
        );
    }
    for rows in [
        vec![vec![1, 0], vec![2]],
        vec![vec![0, 2], vec![1]],
        vec![vec![0, 1], vec![]],
        vec![vec![0, 1, 1], vec![2]],
        vec![vec![0, 9], vec![2]],
        vec![vec![0]],
    ] {
        assert!(
            model
                .with_initial_assignment(&MixedAssignment::new(
                    next.basic().clone(),
                    ListAssignment::new(rows)
                ))
                .is_err()
        );
        assert_eq!(model.initial_assignment(), original);
    }
}

#[test]
fn null_required_pins_allow_partial_construction_but_no_complete_search() {
    let problem = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_pinned(true),
        Variable::new(vec![0, 1]),
    ])
    .unwrap();
    let network = ConstraintNetwork::<_, SimpleScore>::new(2);
    assert_eq!(problem.search_space_size(), Some(0));
    assert!(!problem.is_complete(&problem.initial_assignment()));
    for streamed in [false, true] {
        let config = SolverConfig {
            local_search: vec![],
            ..Default::default()
        };
        let result = if streamed {
            solve_stream(
                &problem,
                || {
                    let mut b = StreamGraphBuilder::<SimpleScore>::new();
                    let s = b.source::<usize, (usize, Option<i32>)>();
                    Ok((b.build(), s))
                },
                config,
            )
            .unwrap()
        } else {
            solve(&problem, &network, config).unwrap()
        };
        assert_eq!(result.assignment.values(), &[None, Some(0)]);
        assert_eq!(result.score, Some(SimpleScore::ZERO.with_structural(-1)));
        assert!(!result.complete);
        assert!(!result.is_optimal());
        assert!(result.solution().is_none());
        let resumed = problem.with_initial_assignment(&result.assignment).unwrap();
        assert_eq!(resumed.search_space_size(), Some(0));
        let exact = exhaustive_search(
            &resumed,
            |_| -> SimpleScore { panic!("no complete assignments") },
            100,
        );
        assert!(exact.best.is_none());
        assert_eq!(exact.evaluations, 0);
        assert!(!exact.is_optimal());
    }
    assert!(
        problem
            .with_initial_assignment(&Assignment::new(vec![Some(0), Some(0)]))
            .is_err()
    );
    // An optional null pin is complete and contributes one fixed choice.
    let optional = FiniteProblem::new(vec![
        Variable::<i32>::new(vec![])
            .allowing_unassigned()
            .with_pinned(true),
    ])
    .unwrap();
    assert!(optional.is_complete(&optional.initial_assignment()));
    assert_eq!(optional.search_space_size(), Some(1));
    // A zero complete space takes precedence over overflow elsewhere.
    let mut vars = vec![Variable::new(vec![0, 1]); 129];
    vars.push(Variable::new(vec![0]).with_pinned(true));
    assert_eq!(
        FiniteProblem::new(vars).unwrap().search_space_size(),
        Some(0)
    );
}

#[test]
fn null_pin_does_not_mask_an_empty_movable_domain() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1]).with_pinned(true),
        Variable::new(vec![]),
    ])
    .unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let r = greedy_construction(&p, &n, ConstructionConfig::default()).unwrap();
    assert_eq!(r.termination, ConstructionTermination::NoCompleteAssignment);
    assert_eq!(r.evaluations, 0);
    assert_eq!(r.assignment, p.initial_assignment());
}
