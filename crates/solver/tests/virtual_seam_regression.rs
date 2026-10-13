//! Original ownership/block/pin/rebase controls for the supported virtual shape.
#[path = "fixtures/virtual_seam_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::Arc;
use timetuck::*;
fn at(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
#[test]
fn owned_gap_blocks_vary_lengths_pins_labels_and_entity_order() {
    for a_len in 4..9 {
        for b_len in 2..7 {
            for pin_a in 0..3 {
                for pin_b in 0..2 {
                    for reverse_owners in [false, true] {
                        let total = a_len + b_len + pin_a + pin_b;
                        let mut v = values(total as u8);
                        for token in &mut v {
                            let value = Arc::get_mut(token).unwrap();
                            value.code = value.code.wrapping_mul(13).wrapping_add(97);
                        }
                        let source_a = (1..=a_len + pin_a).map(|x| x as u8).collect::<Vec<_>>();
                        let source_b = (a_len + pin_a + 1..=total)
                            .map(|x| x as u8)
                            .collect::<Vec<_>>();
                        let (ae, be) = if reverse_owners { (1, 0) } else { (0, 1) };
                        let rows = if reverse_owners {
                            [source_b.as_slice(), source_a.as_slice()]
                        } else {
                            [source_a.as_slice(), source_b.as_slice()]
                        };
                        let pins = if reverse_owners {
                            [pin_b, pin_a]
                        } else {
                            [pin_a, pin_b]
                        };
                        let p = problem(&v, &rows, &pins);
                        let mut state = p.initial_assignment();
                        let left = pin_a + (a_len - 2) / 2;
                        let b_cut = pin_b + (b_len - 2) / 2;
                        let plan = VirtualSeam3OptPlan::prepare(
                            &p,
                            &state,
                            &[
                                (at(ae, pin_a + a_len - 1), at(ae, pin_a)),
                                (at(ae, left), at(ae, left + 1)),
                                (at(be, b_cut), at(be, b_cut + 1)),
                            ],
                            &[(1, 3), (2, 5), (4, 0)],
                        )
                        .unwrap();
                        // Build the signed owner-block algebra independently of the reversal engine.
                        let a = &state.lists()[ae][pin_a..];
                        let b = &state.lists()[be][pin_b..];
                        let l = left - pin_a;
                        let c = b_cut - pin_b;
                        let tour = std::iter::once(a[0].clone())
                            .chain(b[c + 1..].iter().cloned())
                            .chain(a[1..=l].iter().rev().cloned())
                            .chain(a[l + 2..].iter().cloned())
                            .chain(b[..=c].iter().cloned())
                            .chain(std::iter::once(a[l + 1].clone()))
                            .collect::<Vec<_>>();
                        let split = l + b.len() - c - 1;
                        let mut expected = state.lists().to_vec();
                        expected[ae] = state.lists()[ae][..pin_a]
                            .iter()
                            .cloned()
                            .chain(tour[..split].iter().cloned())
                            .collect();
                        expected[be] = state.lists()[be][..pin_b]
                            .iter()
                            .cloned()
                            .chain(tour[split..].iter().cloned())
                            .collect();
                        let saved = state.clone();
                        let movement = plan.movement(&p, &state).unwrap();
                        {
                            let trial = p.apply(&mut state, &movement).unwrap();
                            assert_eq!(trial.lists(), expected);
                            let before_cost = saved
                                .lists()
                                .iter()
                                .map(|r| {
                                    r.windows(2)
                                        .map(|p| p[0].code.abs_diff(p[1].code) as usize)
                                        .sum::<usize>()
                                })
                                .sum::<usize>();
                            let after_cost = trial
                                .lists()
                                .iter()
                                .map(|r| {
                                    r.windows(2)
                                        .map(|p| p[0].code.abs_diff(p[1].code) as usize)
                                        .sum::<usize>()
                                })
                                .sum::<usize>();
                            assert!(before_cost > 0);
                            assert!(after_cost > 0);
                            for value in trial.lists().iter().flatten() {
                                assert!(v.iter().any(|x| Arc::ptr_eq(x, value)));
                            }
                        }
                        assert_eq!(state, saved);
                        p.validate(&state).unwrap();
                    }
                }
            }
        }
    }
}
#[test]
fn declaration_order_directions_and_rebase_keep_real_objects() {
    let v = values(12);
    let p = problem(&v, &[&[1, 7, 8, 2, 3, 9, 10, 6], &[4, 5, 11, 12]], &[0, 0]);
    let a = p.initial_assignment();
    let pairs = [
        (at(0, 7), at(0, 0)),
        (at(0, 3), at(0, 4)),
        (at(1, 0), at(1, 1)),
    ];
    let expected = [
        row(&v, &[1, 5, 11, 12, 2, 8]),
        row(&v, &[7, 9, 10, 6, 4, 3]),
    ];
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        for flags in 0..8 {
            let removed = order
                .iter()
                .enumerate()
                .map(|(i, &pair)| {
                    if flags & (1 << i) != 0 {
                        (pairs[pair].1, pairs[pair].0)
                    } else {
                        pairs[pair]
                    }
                })
                .collect::<Vec<_>>();
            let endpoints = removed
                .iter()
                .flat_map(|&(a, b)| [a, b])
                .collect::<Vec<_>>();
            let index = |p| endpoints.iter().position(|&x| x == p).unwrap();
            let partners = [
                (index(pairs[0].1), index(pairs[1].1)),
                (index(pairs[1].0), index(pairs[2].1)),
                (index(pairs[2].0), index(pairs[0].0)),
            ];
            let plan = VirtualSeam3OptPlan::prepare(&p, &a, &removed, &partners).unwrap();
            let mut actual = a.clone();
            let movement = plan.movement(&p, &actual).unwrap();
            {
                let trial = p.apply(&mut actual, &movement).unwrap();
                assert_eq!(trial.lists(), expected);
            }
            assert_eq!(actual, a);
        }
    }
    let plan = from_edges(
        &p,
        &a,
        &row(&v, &[6, 1, 2, 3, 4, 5]),
        &row(&v, &[1, 3, 2, 5, 4, 6]),
    );
    let dest_values = values(12);
    let dest = problem(
        &dest_values,
        &[&[4, 5, 11, 12], &[1, 7, 8, 2, 3, 9, 10, 6]],
        &[0, 0],
    );
    let mut destination = dest.initial_assignment();
    let rebased = plan
        .rebase(
            &dest,
            &destination,
            |e| Some(1 - e),
            |x| Some(dest_values[usize::from(x.code - 1)].clone()),
        )
        .unwrap();
    let schema = schema();
    let field = schema
        .entity::<ListEntity<Token>>()
        .unwrap()
        .variable("values")
        .unwrap();
    execute(
        &dest,
        &mut destination,
        &rebased,
        field,
        &[
            row(&dest_values, &[7, 9, 10, 6, 4, 3]),
            row(&dest_values, &[1, 5, 11, 12, 2, 8]),
        ],
        &[(1, 0, 8, 6), (0, 0, 4, 6)],
    );
}
#[test]
fn stale_endpoints_receiving_ranges_and_unsupported_shapes_emit_no_events() {
    let v = values(6);
    let p = problem(&v, &[&[1, 2, 3, 6], &[4, 5]], &[0, 0]);
    let mut a = p.initial_assignment();
    let plan = from_edges(
        &p,
        &a,
        &row(&v, &[6, 1, 2, 3, 4, 5]),
        &row(&v, &[1, 3, 2, 5, 4, 6]),
    );
    p.apply(
        &mut a,
        &ListMove::Swap {
            left: at(0, 1),
            right: at(0, 2),
        },
    )
    .unwrap()
    .commit();
    let saved = a.clone();
    let mut events = 0;
    assert!(
        plan.apply_notifying(&p, &mut a, |_, _| events += 1)
            .is_err()
    );
    assert_eq!(events, 0);
    assert_eq!(a, saved);
    let restricted = ListProblem::new_indexed(
        v.clone(),
        vec![
            ListEntity::new(row(&v, &[1, 2, 3, 6])).with_value_range(row(&v, &[1, 2, 3, 6])),
            ListEntity::new(row(&v, &[4, 5])),
        ],
    )
    .unwrap();
    let mut a = restricted.initial_assignment();
    assert!(!plan.is_doable(&restricted, &a));
    assert!(
        plan.apply_notifying(&restricted, &mut a, |_, _| events += 1)
            .is_err()
    );
    assert_eq!(events, 0);
    assert!(
        VirtualSeam3OptPlan::prepare(
            &p,
            &p.initial_assignment(),
            &[
                (at(0, 3), at(0, 0)),
                (at(0, 1), at(0, 2)),
                (at(1, 0), at(1, 1))
            ],
            &[(1, 2), (3, 5), (4, 0)]
        )
        .is_err()
    );
}
#[test]
fn prepared_interior_payload_is_live_and_forward_brackets_record_it() {
    let v = values(12);
    let p = problem(&v, &[&[1, 7, 8, 2, 3, 9, 10, 6], &[4, 5, 11, 12]], &[0, 0]);
    let mut state = p.initial_assignment();
    let plan = from_edges(
        &p,
        &state,
        &row(&v, &[6, 1, 2, 3, 4, 5]),
        &row(&v, &[1, 3, 2, 5, 4, 6]),
    );
    p.apply(
        &mut state,
        &ListMove::Swap {
            left: at(0, 1),
            right: at(0, 2),
        },
    )
    .unwrap()
    .commit();
    let before = state.clone();
    {
        let movement = plan.movement(&p, &state).unwrap();
        let trial = p.apply(&mut state, &movement).unwrap();
        assert_eq!(
            trial.lists(),
            &[
                row(&v, &[1, 5, 11, 12, 2, 7]),
                row(&v, &[8, 9, 10, 6, 4, 3])
            ]
        );
    }
    assert_eq!(state, before);
    let schema = schema();
    let field = schema
        .entity::<ListEntity<Token>>()
        .unwrap()
        .variable("values")
        .unwrap();
    execute(
        &p,
        &mut state,
        &plan,
        field,
        &[
            row(&v, &[1, 5, 11, 12, 2, 7]),
            row(&v, &[8, 9, 10, 6, 4, 3]),
        ],
        &[(0, 0, 8, 6), (1, 0, 4, 6)],
    );
}
#[test]
fn virtual_reconnection_provider_improves_real_list_search_through_both_directors() {
    let p = ListProblem::new_indexed(
        (1..13).collect(),
        vec![
            ListEntity::new(vec![1, 7, 8, 2, 3, 9, 10, 6]),
            ListEntity::new(vec![4, 5, 11, 12]),
        ],
    )
    .unwrap();
    let initial = p.initial_assignment();
    let plan = VirtualSeam3OptPlan::prepare(
        &p,
        &initial,
        &[
            (at(0, 7), at(0, 0)),
            (at(0, 3), at(0, 4)),
            (at(1, 0), at(1, 1)),
        ],
        &[(1, 3), (2, 5), (4, 0)],
    )
    .unwrap();
    let objective = |a: &ListAssignment<i32>| {
        SimpleScore::new(
            -a.lists()
                .iter()
                .map(|row| (row.len() as i64).pow(2))
                .sum::<i64>(),
        )
    };
    let before = objective(&initial);
    let mut network = ListConstraintNetwork::new(2, 12);
    for entity in 0..2 {
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
        let mut generated = 0;
        let mut provider = |p: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| {
            let moves = plan.movement(p, a).ok().into_iter().collect::<Vec<_>>();
            generated += moves.len();
            moves
        };
        let config = LocalSearchConfig {
            max_steps: 3,
            max_move_evaluations: Some(5),
            move_order: MoveOrder::ShuffledMoves {
                seed: 0x145,
                window: std::num::NonZeroUsize::new(2).unwrap(),
            },
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut builder = StreamGraphBuilder::new();
            let sources = ListStreamSources::new(&mut builder);
            builder
                .score("balanced ownership", sources.entities.port(), |(_, row)| {
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
        assert_eq!(result.completed_steps, 1);
        assert_eq!(result.accepted_moves, 1);
        assert_eq!(generated, 1);
        p.validate(&best.assignment).unwrap();
        assert_eq!(p.initial_assignment(), initial);
    }
}
