// Original physical repair controls: no upstream-method compatibility credit.
use timetuck::*;
fn problem(offset: i32) -> ListProblem<i32> {
    ListProblem::new(
        (0..22).map(|i| i + offset).collect(),
        vec![ListEntity::new((0..22).map(|i| i + offset).collect())],
    )
    .unwrap()
}
fn select(
    p: &ListProblem<i32>,
    a: &ListAssignment<i32>,
    offset: i32,
) -> ListKOptRepairProposal<i32> {
    let mut origin = Some(1 + offset);
    let mut values = [13 + offset, 10 + offset, 7 + offset, 4 + offset].into_iter();
    let mut repairs = 0;
    let proposal =
        ListKOptSubtourRepairSelector::new(ListKOptConfig::new(2, 6).unwrap(), vec![1, 1, 1, 1])
            .unwrap()
            .select(
                p,
                a,
                2,
                || origin.take(),
                || values.next(),
                || true,
                |components, _| {
                    repairs += 1;
                    assert_eq!(components.len(), 2);
                    Some((
                        ListPosition {
                            entity: 0,
                            index: 16,
                        },
                        true,
                    ))
                },
            )
            .unwrap();
    assert_eq!(repairs, 1);
    proposal
}
#[test]
fn even_proposals_repair_live_components_and_execute_with_retained_edges_and_undo() {
    for offset in [0, 1000, -500] {
        let p = problem(offset);
        let a = p.initial_assignment();
        let proposal = select(&p, &a, offset);
        assert_eq!(proposal.initial_component_count(), 2);
        assert_eq!(proposal.k(), 6);
        assert_eq!(proposal.endpoint_partners().len(), 12);
        assert_eq!(proposal.physical_plan().physical_cycle_count(), 1);
        let anchor = ListPosition {
            entity: 0,
            index: 0,
        };
        let order = proposal
            .physical_plan()
            .cycle_positions(anchor, false)
            .unwrap();
        let edge = |a: ListPosition, b: ListPosition| {
            if (a.entity, a.index) < (b.entity, b.index) {
                (a, b)
            } else {
                (b, a)
            }
        };
        let mut expected: Vec<_> = (0..22)
            .map(|i| {
                edge(
                    ListPosition {
                        entity: 0,
                        index: i,
                    },
                    ListPosition {
                        entity: 0,
                        index: (i + 1) % 22,
                    },
                )
            })
            .collect();
        for &(a, b) in proposal.removed_edges() {
            let at = expected.iter().position(|&e| e == edge(a, b)).unwrap();
            expected.remove(at);
        }
        let slots: Vec<_> = proposal
            .removed_edges()
            .iter()
            .flat_map(|&(a, b)| [a, b])
            .collect();
        for (i, &j) in proposal.endpoint_partners().iter().enumerate() {
            if i < j {
                expected.push(edge(slots[i], slots[j]));
            }
        }
        let mut observed: Vec<_> = (0..order.len())
            .map(|i| edge(order[i], order[(i + 1) % order.len()]))
            .collect();
        let key = |e: &(ListPosition, ListPosition)| (e.0.entity, e.0.index, e.1.entity, e.1.index);
        expected.sort_by_key(key);
        observed.sort_by_key(key);
        assert_eq!(observed, expected);
        let movement = proposal
            .physical_plan()
            .movement(&p, &a, anchor, false, &[22])
            .unwrap();
        let mut current = a.clone();
        {
            let _undo = p.apply(&mut current, &movement).unwrap();
        }
        assert_eq!(current, a);
        p.apply(&mut current, &movement).unwrap().commit();
        p.validate(&current).unwrap();
        assert_ne!(current, a);
    }
}
#[test]
fn callbacks_reject_budget_and_already_removed_cuts_without_mutating_routes() {
    let p = problem(0);
    let a = p.initial_assignment();
    for maximum in [4, 6] {
        let mut values = [13, 10, 7, 4].into_iter();
        let selector = ListKOptSubtourRepairSelector::new(
            ListKOptConfig::new(2, maximum).unwrap(),
            vec![1, 1, 1],
        )
        .unwrap();
        let result = selector.select(
            &p,
            &a,
            2,
            || Some(1),
            || values.next(),
            || true,
            |_, _| {
                Some((
                    ListPosition {
                        entity: 0,
                        index: 1,
                    },
                    true,
                ))
            },
        );
        assert!(matches!(
            result,
            Err(ListKOptRepairError::RepairBudget | ListKOptRepairError::InvalidRepairChoice)
        ));
        assert_eq!(p.initial_assignment(), a);
    }
    let selector =
        ListKOptSubtourRepairSelector::new(ListKOptConfig::new(2, 6).unwrap(), vec![1, 1, 1, 1])
            .unwrap();
    assert!(matches!(
        selector.select(
            &p,
            &a,
            4,
            || panic!(),
            || panic!(),
            || panic!(),
            |_, _| panic!()
        ),
        Err(ListKOptRepairError::InvalidDraw)
    ));
}
#[test]
fn live_payloads_and_pins_refresh_position_resolution() {
    let p = problem(0);
    let a = p.initial_assignment();
    let q = select(&p, &a, 0);
    let movement = q
        .physical_plan()
        .movement(
            &p,
            &a,
            ListPosition {
                entity: 0,
                index: 0,
            },
            false,
            &[22],
        )
        .unwrap();
    let mut b = a.clone();
    p.apply(&mut b, &movement).unwrap().commit();
    assert!(
        q.physical_plan()
            .movement(
                &p,
                &b,
                ListPosition {
                    entity: 0,
                    index: 0
                },
                false,
                &[22]
            )
            .is_err()
    );
    let pinned = ListProblem::new(
        (0..22).collect(),
        vec![ListEntity::new((0..22).collect()).with_pinned_prefix(2)],
    )
    .unwrap();
    let before = pinned.initial_assignment();
    assert!(
        ListKOptSubtourRepairSelector::new(ListKOptConfig::new(4, 6).unwrap(), vec![1])
            .unwrap()
            .select(
                &pinned,
                &before,
                0,
                || Some(1),
                || Some(13),
                || true,
                |_, _| None
            )
            .is_err()
    );
}

fn generated(
    p: &ListProblem<i32>,
    before: &ListAssignment<i32>,
    k: usize,
    shift: usize,
    reverse: bool,
) -> Result<ListKOptRepairProposal<i32>, ListKOptRepairError> {
    let flat: Vec<_> = before
        .lists()
        .iter()
        .enumerate()
        .flat_map(|(entity, row)| {
            (p.entities()[entity].pinned_prefix()..row.len())
                .map(move |index| ListPosition { entity, index })
        })
        .collect();
    let cut_indices: Vec<_> = (0..k).map(|i| shift + 3 * i).collect();
    let mut order = vec![cut_indices[0]];
    order.extend(cut_indices[1..].iter().rev().copied());
    let values: Vec<_> = order
        .iter()
        .map(|&i| before.lists()[flat[i].entity][flat[i].index])
        .collect();
    let mut choices = values.into_iter();
    let origin = choices.next().unwrap();
    let probe =
        before.lists()[flat[(shift + 1) % flat.len()].entity][flat[(shift + 1) % flat.len()].index];
    let mut choices = choices.chain(std::iter::repeat(probe));
    ListKOptSubtourRepairSelector::new(ListKOptConfig::new(k, k + 2 * (k - 1)).unwrap(), vec![1])
        .unwrap()
        .select(
            p,
            before,
            0,
            || Some(origin),
            || choices.next(),
            || !reverse,
            |components, removed| {
                for component in components.iter().rev() {
                    for &a in component {
                        let i = flat.iter().position(|&x| x == a).unwrap();
                        let b = flat[(i + 1) % flat.len()];
                        if component.contains(&b)
                            && !removed
                                .iter()
                                .any(|&(x, y)| (x == a && y == b) || (x == b && y == a))
                        {
                            return Some((a, true));
                        }
                    }
                }
                None
            },
        )
}
#[test]
fn bounded_even_k_geometry_and_reversed_draws_produce_real_connected_tours() {
    for k in [4, 6, 8, 10] {
        for extra in [6, 11] {
            for reverse in [false, true] {
                let n = 3 * k + extra;
                let p = ListProblem::new_indexed(
                    (0..n as i32).collect(),
                    vec![ListEntity::new((0..n as i32).collect())],
                )
                .unwrap();
                let before = p.initial_assignment();
                let q = generated(&p, &before, k, 1, reverse)
                    .unwrap_or_else(|e| panic!("k={k} extra={extra} reverse={reverse}: {e}"));
                assert!(q.initial_component_count() > 1);
                assert_eq!(q.physical_plan().physical_cycle_count(), 1);
                assert!(q.k() > k);
                assert_eq!(q.active_endpoint_partners().len(), 2 * q.k());
                let first = ListPosition {
                    entity: 0,
                    index: 0,
                };
                let movement = q
                    .physical_plan()
                    .movement(&p, &before, first, false, &[n])
                    .unwrap();
                let mut current = before.clone();
                {
                    let _undo = p.apply(&mut current, &movement).unwrap();
                }
                assert_eq!(current, before);
                p.apply(&mut current, &movement).unwrap().commit();
                p.validate(&current).unwrap();
            }
        }
    }
}
#[test]
fn multiowner_wrap_and_prefixes_follow_combined_original_adjacency() {
    let p = ListProblem::new_indexed(
        (0..40).collect(),
        vec![
            ListEntity::new((0..14).collect()).with_pinned_prefix(2),
            ListEntity::new((14..40).collect()).with_pinned_prefix(3),
        ],
    )
    .unwrap();
    let before = p.initial_assignment();
    // Combined cut at row0's end must point to row1's first movable value.
    let mut choices = [25, 22, 19].into_iter().chain(std::iter::repeat(20));
    let q = ListKOptSubtourRepairSelector::new(ListKOptConfig::new(4, 10).unwrap(), vec![1])
        .unwrap()
        .select(
            &p,
            &before,
            0,
            || Some(13),
            || choices.next(),
            || true,
            |components, removed| {
                for c in components {
                    for &a in c {
                        let b = if a.entity == 0 && a.index == 13 {
                            ListPosition {
                                entity: 1,
                                index: 3,
                            }
                        } else if a.entity == 1 && a.index == 25 {
                            ListPosition {
                                entity: 0,
                                index: 2,
                            }
                        } else {
                            ListPosition {
                                entity: a.entity,
                                index: a.index + 1,
                            }
                        };
                        if c.contains(&b)
                            && !removed
                                .iter()
                                .any(|&(x, y)| (x == a && y == b) || (x == b && y == a))
                        {
                            return Some((a, true));
                        }
                    }
                }
                None
            },
        )
        .unwrap();
    assert_eq!(
        q.removed_edges()[0],
        (
            ListPosition {
                entity: 0,
                index: 13
            },
            ListPosition {
                entity: 1,
                index: 3
            }
        )
    );
    assert_eq!(&q.removed_values()[..2], &[13, 17]);
    let movement = q
        .physical_plan()
        .movement(
            &p,
            &before,
            ListPosition {
                entity: 0,
                index: 2,
            },
            false,
            &[12, 23],
        )
        .unwrap();
    let mut after = before.clone();
    p.apply(&mut after, &movement).unwrap().commit();
    assert_eq!(&after.lists()[0][..2], &[0, 1]);
    assert_eq!(&after.lists()[1][..3], &[14, 15, 16]);
    p.validate(&after).unwrap();
}
fn rank_score(row: &[i32]) -> SimpleScore {
    SimpleScore::new(
        -row.iter()
            .enumerate()
            .map(|(i, &v)| i as i64 * i64::from(v))
            .sum::<i64>(),
    )
}
#[derive(Default)]
struct RepairProvider {
    opened: Vec<ListAssignment<i32>>,
    successful: usize,
}
impl MoveProvider<ListProblem<i32>, ListAssignment<i32>, ListMove<i32>> for RepairProvider {
    fn moves<'a>(
        &'a mut self,
        p: &'a ListProblem<i32>,
        before: &'a ListAssignment<i32>,
        _: u64,
    ) -> Box<dyn Iterator<Item = ListMove<i32>> + 'a>
    where
        ListMove<i32>: 'a,
    {
        self.opened.push(before.clone());
        let mut moves = vec![];
        for shift in 1..=4 {
            if let Ok(q) = generated(p, before, 4, shift, false)
                && let Ok(m) = q.physical_plan().movement(
                    p,
                    before,
                    ListPosition {
                        entity: 0,
                        index: 0,
                    },
                    false,
                    &[before.lists()[0].len()],
                )
            {
                self.successful += 1;
                moves.push(m);
            }
        }
        Box::new(moves.into_iter())
    }
}
#[test]
fn active_graph_repairs_feed_dense_and_stream_search_on_current_assignments() {
    let p = ListProblem::new_indexed((0..28).collect(), vec![ListEntity::new((0..28).collect())])
        .unwrap();
    let before = p.initial_assignment();
    let initial = rank_score(&before.lists()[0]);
    let config = LocalSearchConfig {
        max_steps: 3,
        max_evaluations: 20,
        selection: MoveSelection::BestAccepted,
        acceptance: Acceptance::StrictImprovement,
        assert_scores: true,
        ..Default::default()
    };
    let mut dense = ListConstraintNetwork::new(1, 28);
    dense
        .add_constraint("rank", vec![ListScope::Entity(0)], |facts| {
            let ListFact::Entity(row) = facts[0] else {
                unreachable!()
            };
            rank_score(row)
        })
        .unwrap();
    let mut dp = RepairProvider::default();
    let d =
        list_local_search_from_with_provider(&p, &dense, before.clone(), config.clone(), &mut dp)
            .unwrap();
    let mut builder = StreamGraphBuilder::new();
    let sources = ListStreamSources::new(&mut builder);
    builder
        .score("rank", sources.entities.port(), |(_, row)| rank_score(row))
        .unwrap();
    let mut sp = RepairProvider::default();
    let s = list_stream_local_search_from_with_provider(
        &p,
        builder.build(),
        sources,
        before.clone(),
        config,
        &mut sp,
    )
    .unwrap();
    assert_eq!(d, s);
    assert_eq!(dp.opened, sp.opened);
    assert!(dp.successful > 0);
    assert!(dp.opened.len() > 1);
    assert_ne!(dp.opened[0], dp.opened[1]);
    let best = d.best.unwrap();
    assert!(best.score > initial);
    p.validate(&best.assignment).unwrap();
    assert!(d.evaluations > 0 && d.evaluations <= 20);
}
