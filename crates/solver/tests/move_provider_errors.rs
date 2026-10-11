//! Original validation and invalid-candidate regressions across all provider entry points.
use timetuck::*;
fn basic() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap()
}
fn lists() -> ListProblem<i32> {
    ListProblem::new(vec![7], vec![ListEntity::new(vec![7])]).unwrap()
}

#[test]
fn invalid_models_and_foreign_graphs_fail_before_provider_work() {
    {
        let p = basic();
        let c = LocalSearchConfig {
            max_evaluations: 0,
            ..Default::default()
        };
        let mut provider = |_: &FiniteProblem<i32>,
                            _: &Assignment<i32>,
                            _: u64|
         -> Vec<Move<i32>> { panic!("provider must not run") };
        assert!(
            local_search_from_with_provider(
                &p,
                &ConstraintNetwork::<_, SimpleScore>::new(2),
                p.initial_assignment(),
                c.clone(),
                &mut provider
            )
            .is_err()
        );
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let source = builder.source::<usize, (usize, Option<i32>)>();
        assert!(
            stream_local_search_from_with_provider(
                &p,
                StreamGraphBuilder::<SimpleScore>::new().build(),
                source,
                p.initial_assignment(),
                c,
                &mut provider
            )
            .is_err()
        );
    }
    {
        let p = lists();
        let c = LocalSearchConfig {
            max_evaluations: 0,
            ..Default::default()
        };
        let mut provider = |_: &ListProblem<i32>,
                            _: &ListAssignment<i32>,
                            _: u64|
         -> Vec<ListMove<i32>> { panic!("provider must not run") };
        assert!(
            list_local_search_from_with_provider(
                &p,
                &ListConstraintNetwork::<_, SimpleScore>::new(2, 1),
                p.initial_assignment(),
                c.clone(),
                &mut provider
            )
            .is_err()
        );
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let source = ListStreamSources::<i32>::new(&mut builder);
        assert!(
            list_stream_local_search_from_with_provider(
                &p,
                StreamGraphBuilder::<SimpleScore>::new().build(),
                source,
                p.initial_assignment(),
                c,
                &mut provider
            )
            .is_err()
        );
    }
    {
        let p = MixedProblem::new(basic(), lists());
        let c = MixedLocalSearchConfig {
            search: LocalSearchConfig {
                max_evaluations: 0,
                ..Default::default()
            },
            joint_moves: true,
        };
        let mut provider = |_: &MixedProblem<i32, i32>,
                            _: &MixedAssignment<i32, i32>,
                            _: u64|
         -> Vec<MixedMove<i32, i32>> { panic!("provider must not run") };
        assert!(
            mixed_local_search_from_with_provider(
                &p,
                &MixedConstraintNetwork::<_, _, SimpleScore>::new(2, 1, 1),
                p.initial_assignment(),
                c.clone(),
                &mut provider
            )
            .is_err()
        );
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let source = MixedStreamSources::<i32, i32>::new(&mut builder);
        assert!(
            mixed_stream_local_search_from_with_provider(
                &p,
                StreamGraphBuilder::<SimpleScore>::new().build(),
                source,
                p.initial_assignment(),
                c,
                &mut provider
            )
            .is_err()
        );
    }
}

#[test]
fn invalid_custom_candidates_propagate_errors_after_a_valid_candidate() {
    {
        let p = basic();
        let c = LocalSearchConfig {
            selection: MoveSelection::BestAccepted,
            ..Default::default()
        };
        let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
            vec![
                Move::Composite(vec![]),
                Move::Change {
                    variable: 99,
                    value: Some(1),
                },
            ]
        };
        let error = local_search_from_with_provider(
            &p,
            &ConstraintNetwork::<_, SimpleScore>::new(1),
            p.initial_assignment(),
            c.clone(),
            &mut provider,
        )
        .unwrap_err();
        assert!(error.to_string().contains("99"), "{error}");
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let source = builder.source::<usize, (usize, Option<i32>)>();
        let error = stream_local_search_from_with_provider(
            &p,
            builder.build(),
            source,
            p.initial_assignment(),
            c,
            &mut provider,
        )
        .unwrap_err();
        assert!(error.to_string().contains("99"), "{error}");
    }
    {
        let p = lists();
        let c = LocalSearchConfig {
            selection: MoveSelection::BestAccepted,
            ..Default::default()
        };
        let mut provider = |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64| {
            vec![
                ListMove::Composite(vec![]),
                ListMove::Swap {
                    left: ListPosition {
                        entity: 99,
                        index: 0,
                    },
                    right: ListPosition {
                        entity: 0,
                        index: 0,
                    },
                },
            ]
        };
        let error = list_local_search_from_with_provider(
            &p,
            &ListConstraintNetwork::<_, SimpleScore>::new(1, 1),
            p.initial_assignment(),
            c.clone(),
            &mut provider,
        )
        .unwrap_err();
        assert!(error.to_string().contains("99"), "{error}");
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let source = ListStreamSources::<i32>::new(&mut builder);
        let error = list_stream_local_search_from_with_provider(
            &p,
            builder.build(),
            source,
            p.initial_assignment(),
            c,
            &mut provider,
        )
        .unwrap_err();
        assert!(error.to_string().contains("99"), "{error}");
    }
    {
        let p = MixedProblem::new(basic(), lists());
        let c = MixedLocalSearchConfig {
            search: LocalSearchConfig {
                selection: MoveSelection::BestAccepted,
                ..Default::default()
            },
            joint_moves: true,
        };
        let mut provider = |_: &MixedProblem<i32, i32>, _: &MixedAssignment<i32, i32>, _: u64| {
            vec![
                MixedMove {
                    basic: Move::Composite(vec![]),
                    lists: ListMove::Composite(vec![]),
                },
                MixedMove {
                    basic: Move::Change {
                        variable: 99,
                        value: Some(1),
                    },
                    lists: ListMove::Composite(vec![]),
                },
            ]
        };
        let error = mixed_local_search_from_with_provider(
            &p,
            &MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 1, 1),
            p.initial_assignment(),
            c.clone(),
            &mut provider,
        )
        .unwrap_err();
        assert!(error.to_string().contains("99"), "{error}");
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let source = MixedStreamSources::<i32, i32>::new(&mut builder);
        let error = mixed_stream_local_search_from_with_provider(
            &p,
            builder.build(),
            source,
            p.initial_assignment(),
            c,
            &mut provider,
        )
        .unwrap_err();
        assert!(error.to_string().contains("99"), "{error}");
    }
}
