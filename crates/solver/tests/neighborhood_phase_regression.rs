//! Original phase-builder integration cases; no upstream method credit.
use std::cell::Cell;
use timetuck::*;
fn config() -> LocalSearchConfig<'static, SimpleScore> {
    LocalSearchConfig {
        max_steps: 1,
        assert_scores: true,
        selection: MoveSelection::BestAccepted,
        ..Default::default()
    }
}
#[test]
fn opted_in_provider_is_installed_at_actual_phase_index_and_drives_both_basic_backends() {
    let model = FiniteProblem::new(vec![Variable::new(vec![0i64, 1, 2]).with_initial(0)]).unwrap();
    let mut network = ConstraintNetwork::new(1);
    network
        .add_constraint("value", vec![0], |r| SimpleScore::new(*r[0].unwrap()))
        .unwrap();
    let control = solve(
        &model,
        &network,
        SolverSequence {
            limits: Default::default(),
            phases: vec![SolverPhase::LocalSearch(config())],
        },
    )
    .unwrap();
    assert_eq!(control.assignment.values(), &[Some(2)]);
    for stream in [false, true] {
        let calls = Cell::new(0);
        let mut provider = |_: &FiniteProblem<i64>, a: &Assignment<i64>, step| {
            assert_eq!(step, 0);
            assert_eq!(a.values(), &[Some(0)]);
            calls.set(calls.get() + 1);
            vec![Move::Change {
                variable: 0,
                value: Some(1),
            }]
        };
        let mut plan = SolverSequence {
            limits: Default::default(),
            phases: vec![SolverPhase::Construction(Default::default())],
        };
        let mut providers = PhaseMoveProviders::default();
        let prepared = NeighborhoodPhase::local_search(config())
            .prepare(NeighborhoodApi { enabled: true }, Some(&mut provider))
            .unwrap();
        assert_eq!(prepared.append_to(&mut plan, &mut providers).unwrap(), 1);
        assert!(providers.contains(1));
        let result = if stream {
            solve_stream_with_extensions(
                &model,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let source = b.source::<usize, (usize, Option<i64>)>();
                    b.score("value", source.port(), |(_, v)| {
                        SimpleScore::new(v.unwrap())
                    })
                    .unwrap();
                    Ok((b.build(), source))
                },
                plan,
                SolverExtensions {
                    move_providers: providers,
                    ..Default::default()
                },
            )
            .unwrap()
        } else {
            solve_with_extensions(
                &model,
                &network,
                plan,
                SolverExtensions {
                    move_providers: providers,
                    ..Default::default()
                },
            )
            .unwrap()
        };
        assert_eq!(result.assignment.values(), &[Some(1)]); // Built-in enumeration would choose 2.
        assert_eq!(result.score, Some(SimpleScore::new(1)));
        assert_eq!(result.work.completed_steps, 1);
        assert_eq!(calls.get(), 1);
    }
}
#[test]
fn same_builder_installs_actual_list_provider_in_both_backends() {
    let model = ListProblem::new(
        vec![7i64],
        vec![
            ListEntity::new(vec![7]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let mut network = ListConstraintNetwork::new(3, 1);
    network
        .add_constraint("owner", vec![ListScope::Value(0)], |r| {
            let ListFact::Value { element, .. } = &r[0] else {
                unreachable!()
            };
            SimpleScore::new(element.as_ref().unwrap().position.entity as i64)
        })
        .unwrap();
    let control = solve_list(
        &model,
        &network,
        SolverSequence {
            limits: Default::default(),
            phases: vec![SolverPhase::LocalSearch(config())],
        },
    )
    .unwrap();
    assert_eq!(control.assignment.location(&7).unwrap().entity, 2);
    for stream in [false, true] {
        let calls = Cell::new(0);
        let mut provider = |_: &ListProblem<i64>, a: &ListAssignment<i64>, _| {
            assert_eq!(a.location(&7).unwrap().entity, 0);
            calls.set(calls.get() + 1);
            vec![ListMove::Change {
                source: ListPosition {
                    entity: 0,
                    index: 0,
                },
                destination: ListPosition {
                    entity: 1,
                    index: 0,
                },
            }]
        };
        let mut plan = SolverSequence {
            limits: Default::default(),
            phases: vec![],
        };
        let mut providers = PhaseMoveProviders::default();
        NeighborhoodPhase::local_search(config())
            .prepare(NeighborhoodApi { enabled: true }, Some(&mut provider))
            .unwrap()
            .append_to(&mut plan, &mut providers)
            .unwrap();
        let result = if stream {
            solve_list_stream_with_extensions(
                &model,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let sources = ListStreamSources::new(&mut b);
                    b.score("owner", sources.values.port(), |v| {
                        SimpleScore::new(v.position.unwrap().entity as i64)
                    })
                    .unwrap();
                    Ok((b.build(), sources))
                },
                plan,
                SolverExtensions {
                    move_providers: providers,
                    ..Default::default()
                },
            )
            .unwrap()
        } else {
            solve_list_with_extensions(
                &model,
                &network,
                plan,
                SolverExtensions {
                    move_providers: providers,
                    ..Default::default()
                },
            )
            .unwrap()
        };
        assert_eq!(result.assignment.location(&7).unwrap().entity, 1);
        assert_eq!(result.score, Some(SimpleScore::new(1)));
        assert_eq!(calls.get(), 1);
    }
}
#[test]
fn descent_compiles_real_controls_preserves_limits_and_runs_without_preview_opt_in() {
    let union = NeighborhoodUnion {
        children: &[(Neighborhood::Change, 1), (Neighborhood::Swap, 1)],
        order: UnionOrder::Sequential,
    };
    let request = NeighborhoodPhase::variable_descent(
        &union,
        LocalSearchConfig {
            max_steps: 9,
            max_evaluations: 100,
            best_score_limit: Some(SimpleScore::new(2)),
            assert_scores: true,
            ..Default::default()
        },
    )
    .unwrap();
    let prepared = request
        .prepare::<FiniteProblem<i64>, Assignment<i64>, Move<i64>>(NeighborhoodApi::default(), None)
        .unwrap();
    assert_eq!(prepared.config().max_steps, 9);
    assert_eq!(prepared.config().max_evaluations, 100);
    assert_eq!(prepared.config().selection, MoveSelection::FirstAccepted);
    assert_eq!(prepared.config().move_order, MoveOrder::Original);
    assert!(prepared.config().assert_scores);
    let model = FiniteProblem::new(vec![Variable::new(vec![0i64, 1, 2]).with_initial(0)]).unwrap();
    let mut network = ConstraintNetwork::new(1);
    network
        .add_constraint("value", vec![0], |r| SimpleScore::new(*r[0].unwrap()))
        .unwrap();
    let mut plan = SolverSequence {
        limits: Default::default(),
        phases: vec![],
    };
    let mut providers = PhaseMoveProviders::default();
    prepared.append_to(&mut plan, &mut providers).unwrap();
    let result = solve_with_extensions(
        &model,
        &network,
        plan,
        SolverExtensions {
            move_providers: providers,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.assignment.values(), &[Some(2)]);
    assert_eq!(result.score, Some(SimpleScore::new(2)));
    assert_eq!(result.work.completed_steps, 2);
}
#[test]
fn binding_conflict_is_atomic_and_descent_requires_ordered_union() {
    let mut a = |_: &FiniteProblem<i64>, _: &Assignment<i64>, _| Vec::<Move<i64>>::new();
    let mut b = |_: &FiniteProblem<i64>, _: &Assignment<i64>, _| Vec::<Move<i64>>::new();
    let mut plan = SolverSequence {
        limits: Default::default(),
        phases: vec![],
    };
    let mut providers = PhaseMoveProviders::default();
    providers.bind(0, &mut a);
    let prepared = NeighborhoodPhase::local_search(config())
        .prepare(NeighborhoodApi { enabled: true }, Some(&mut b))
        .unwrap();
    assert!(matches!(
        prepared.append_to(&mut plan, &mut providers),
        Err(MoveProviderBindingError::AlreadyBound { phase: 0 })
    ));
    assert!(plan.phases.is_empty());
    assert!(providers.contains(0));
    let union = NeighborhoodUnion {
        children: &[(Neighborhood::Change, 1)],
        order: UnionOrder::Random { seed: 1 },
    };
    assert!(NeighborhoodPhase::variable_descent(&union, config()).is_err());
}
