use timetuck::*;

fn model(n: usize, m: usize, pin: usize, reverse: bool) -> ListProblem<i32> {
    let values: Vec<_> = (0..n + m + 2 * pin + 1)
        .map(|i| 103 + 17 * i as i32)
        .collect();
    let split = n + pin;
    let mut rows = vec![
        ListEntity::new(values[..split].to_vec()).with_pinned_prefix(pin),
        ListEntity::new(values[split..values.len() - 1].to_vec()).with_pinned_prefix(pin),
    ];
    if reverse {
        rows.reverse();
    }
    rows.push(ListEntity::new(vec![*values.last().unwrap()]).with_pinned(true));
    ListProblem::new_indexed(values, rows).unwrap()
}
#[test]
fn live_discovery_varies_shapes_pins_owner_order_and_restores_all_payloads() {
    for n in 4..9 {
        for m in 2..6 {
            for pin in 0..3 {
                for reverse in [false, true] {
                    let p = model(n, m, pin, reverse);
                    let mut state = p.initial_assignment();
                    let before = state.clone();
                    let plans: Vec<_> = discover_virtual_seam_three_opt(&p, &state, usize::MAX)
                        .unwrap()
                        .collect();
                    let expected = (n - 3) * (m - 1) + m.saturating_sub(3) * (n - 1);
                    assert_eq!(plans.len(), expected);
                    for plan in plans {
                        let movement = plan.movement(&p, &state).unwrap();
                        {
                            let trial = p.apply(&mut state, &movement).unwrap();
                            p.validate(&trial).unwrap();
                            assert_ne!(*trial, before);
                            for e in 0..2 {
                                assert_eq!(&trial.lists()[e][..pin], &before.lists()[e][..pin]);
                            }
                            assert_eq!(trial.lists()[2], before.lists()[2]);
                            let mut original: Vec<_> =
                                before.lists().iter().flatten().copied().collect();
                            let mut changed: Vec<_> =
                                trial.lists().iter().flatten().copied().collect();
                            original.sort();
                            changed.sort();
                            assert_eq!(original, changed);
                        }
                        assert_eq!(state, before);
                    }
                    let mut provider = VirtualSeam3OptProvider::new(1);
                    assert_eq!(provider.moves(&p, &state, 0).count(), 1);
                    let movement = provider.moves(&p, &state, 0).next().unwrap();
                    p.apply(&mut state, &movement).unwrap().commit();
                    let direct: Vec<_> = discover_virtual_seam_three_opt(&p, &state, usize::MAX)
                        .unwrap()
                        .map(|plan| plan.movement(&p, &state).unwrap())
                        .collect();
                    let mut provider = VirtualSeam3OptProvider::new(usize::MAX);
                    assert_eq!(provider.moves(&p, &state, 1).collect::<Vec<_>>(), direct);
                }
            }
        }
    }
}
#[test]
fn rejects_receiving_ranges_and_validates_even_disabled_discovery() {
    let p = ListProblem::new_indexed(
        (0..6).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3]).with_value_range(vec![0, 1, 2, 3]),
            ListEntity::new(vec![4, 5]).with_value_range(vec![4, 5]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    assert_eq!(
        discover_virtual_seam_three_opt(&p, &a, 100)
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        discover_virtual_seam_three_opt(&p, &a, 0).unwrap().count(),
        0
    );
    let foreign =
        ListProblem::new_indexed((0..6).collect(), vec![ListEntity::new((0..6).collect())])
            .unwrap();
    assert!(discover_virtual_seam_three_opt(&p, &foreign.initial_assignment(), 0).is_err());
    let p = ListProblem::new_indexed(
        (0..6).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3]).with_pinned(true),
            ListEntity::new(vec![4, 5]),
        ],
    )
    .unwrap();
    assert_eq!(
        discover_virtual_seam_three_opt(&p, &p.initial_assignment(), 100)
            .unwrap()
            .count(),
        0
    );
}
fn cycle_edges(rows: &[Vec<i32>]) -> std::collections::BTreeSet<(i32, i32)> {
    let values: Vec<_> = rows.iter().take(2).flatten().copied().collect();
    values
        .iter()
        .copied()
        .zip(values.iter().copied().cycle().skip(1))
        .take(values.len())
        .map(|(a, b)| if a < b { (a, b) } else { (b, a) })
        .collect()
}
#[test]
fn declared_three_cut_discovery_reaches_beyond_physical_three_cut_cycle_space() {
    let p = model(4, 2, 0, false);
    let mut a = p.initial_assignment();
    let before = cycle_edges(a.lists());
    let plan = discover_virtual_seam_three_opt(&p, &a, 1)
        .unwrap()
        .next()
        .unwrap();
    let movement = plan.movement(&p, &a).unwrap();
    let trial = p.apply(&mut a, &movement).unwrap();
    let after = cycle_edges(trial.lists());
    assert_eq!(before.difference(&after).count(), 4);
    // A physical three-cut reconnection retains every one of the other cycle
    // edges, regardless of its rotation or weak-composition partition. Thus
    // this result cannot belong to the old combined-cycle k<=3 proposal space.
}
#[test]
fn discovered_provider_and_base_union_improve_dense_and_stream_search() {
    let p = model(8, 4, 0, false);
    let initial = p.initial_assignment();
    let objective = |a: &ListAssignment<i32>| {
        SimpleScore::new(
            -a.lists()
                .iter()
                .map(|r| (r.len() as i64).pow(2))
                .sum::<i64>(),
        )
    };
    let before = objective(&initial);
    let mut network = ListConstraintNetwork::new(3, 13);
    for entity in 0..3 {
        network
            .add_constraint(
                format!("load {entity}"),
                vec![ListScope::Entity(entity)],
                |facts| {
                    let ListFact::Entity(row) = facts[0] else {
                        unreachable!()
                    };
                    SimpleScore::new(-(row.len() as i64).pow(2))
                },
            )
            .unwrap();
    }
    for stream in [false, true] {
        let mut discovered = VirtualSeam3OptProvider::new(128);
        let mut base = DirectedTwoOptProvider::new(false);
        let mut provider = ProviderUnion::unweighted(
            [
                &mut discovered
                    as &mut dyn MoveProvider<ListProblem<i32>, ListAssignment<i32>, ListMove<i32>>,
                &mut base,
            ],
            UnionOrder::Sequential,
        )
        .unwrap();
        assert_eq!(provider.neighborhood_count().unwrap(), 2);
        let config = LocalSearchConfig {
            max_steps: 3,
            max_move_evaluations: Some(24),
            move_order: MoveOrder::ShuffledMoves {
                seed: 147,
                window: std::num::NonZeroUsize::new(2).unwrap(),
            },
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut builder = StreamGraphBuilder::new();
            let sources = ListStreamSources::new(&mut builder);
            builder
                .score("load", sources.entities.port(), |(_, row)| {
                    SimpleScore::new(-(row.len() as i64).pow(2))
                })
                .unwrap();
            list_stream_local_search_from_with_provider(
                &p,
                builder.build(),
                sources,
                initial.clone(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            list_local_search_from_with_provider(
                &p,
                &network,
                initial.clone(),
                config,
                &mut provider,
            )
            .unwrap()
        };
        let best = result.best.unwrap();
        assert!(best.score > before);
        assert_eq!(best.score, objective(&best.assignment));
        p.validate(&best.assignment).unwrap();
        assert_eq!(best.assignment.lists()[2], initial.lists()[2]);
        assert!(result.accepted_moves > 0);
        assert!(result.completed_steps <= 3);
    }
}

#[test]
fn discovery_preserves_assigned_allocations_and_obeys_topology_budget() {
    use std::sync::Arc;
    let assigned: Vec<_> = (0..9).map(|i| Arc::new(format!("task-{i}"))).collect();
    // Registry objects are equal, but independently allocated. Moves must carry
    // the actual assigned objects, rather than replacing them from the registry.
    let registry = assigned.iter().map(|v| Arc::new((**v).clone())).collect();
    let p = ListProblem::new_indexed(
        registry,
        vec![
            ListEntity::new(assigned[..6].to_vec()),
            ListEntity::new(assigned[6..].to_vec()),
        ],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    for budget in 0..7 {
        assert_eq!(
            discover_virtual_seam_three_opt(&p, &a, budget)
                .unwrap()
                .count(),
            budget.min(6)
        );
    }
    let movement = VirtualSeam3OptProvider::new(1)
        .moves(&p, &a, 0)
        .next()
        .unwrap();
    {
        let trial = p.apply(&mut a, &movement).unwrap();
        for value in trial.lists().iter().flatten() {
            let original = assigned.iter().find(|x| *x == value).unwrap();
            assert!(Arc::ptr_eq(value, original));
        }
    }
    for (value, original) in a.lists().iter().flatten().zip(&assigned) {
        assert!(Arc::ptr_eq(value, original));
    }
}
