//! Original sparse receiver-index differential and solver coverage.
use timetuck::*;
fn models(optional: bool) -> (ListProblem<usize>, ListProblem<usize>) {
    let n = 80;
    let values: Vec<_> = (0..2 * n + 5).rev().collect();
    let entities: Vec<_> = (0..n)
        .map(|e| {
            let initial = if e % 10 == 0 {
                vec![]
            } else {
                vec![2 * e + 1, 2 * e]
            };
            let mut row = ListEntity::new(initial);
            if e % 11 == 0 {
                row = row.with_pinned(true);
            } else if e % 7 == 0 && e % 10 != 0 {
                row = row.with_pinned_prefix(1);
            }
            if e % 13 != 0 {
                row = row.with_value_range(
                    [e, (e + 1) % n, (e + 3) % n]
                        .into_iter()
                        .flat_map(|g| [2 * g + 1, 2 * g])
                        .chain((0..5).filter(|i| i % 2 == e % 2).map(|i| 2 * n + i))
                        .collect(),
                );
            }
            row
        })
        .collect();
    let pair = (
        ListProblem::new_indexed(values.clone(), entities.clone()).unwrap(),
        ListProblem::new(values, entities).unwrap(),
    );
    if optional {
        (pair.0.allowing_unassigned(), pair.1.allowing_unassigned())
    } else {
        pair
    }
}
#[test]
fn sparse_swaps_match_direct_order_for_asymmetric_ranges_pins_and_optional_replacements() {
    for optional in [false, true] {
        let (indexed, direct) = models(optional);
        let a = indexed.initial_assignment();
        for family in [
            Neighborhood::Swap,
            Neighborhood::UniqueSwap,
            Neighborhood::ChangeAndSwap,
            Neighborhood::ChangeAndUniqueSwap,
        ] {
            let actual = list_moves(&indexed, &a, family.clone())
                .unwrap()
                .collect::<Vec<_>>();
            let expected = list_moves(&direct, &a, family.clone())
                .unwrap()
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "optional={optional} family={family:?}");
            for limit in [0, 1, 7, 80] {
                assert_eq!(
                    list_moves(&indexed, &a, family.clone())
                        .unwrap()
                        .take(limit)
                        .collect::<Vec<_>>(),
                    expected.iter().take(limit).cloned().collect::<Vec<_>>()
                );
            }
        }
    }
}
#[test]
fn each_new_scan_follows_commits_and_rollbacks_without_retaining_receiver_state() {
    let (indexed, direct) = models(true);
    let mut assignment = indexed.initial_assignment();
    for step in 0..16 {
        let candidates = list_unique_swap_moves(&indexed, &assignment)
            .unwrap()
            .collect::<Vec<_>>();
        assert_eq!(
            candidates,
            list_unique_swap_moves(&direct, &assignment)
                .unwrap()
                .collect::<Vec<_>>()
        );
        let m = candidates[(step * 71) % candidates.len()].clone();
        let original = assignment.clone();
        {
            let transaction = indexed.apply(&mut assignment, &m).unwrap();
            assert_eq!(
                list_swap_moves(&indexed, &transaction)
                    .unwrap()
                    .collect::<Vec<_>>(),
                list_swap_moves(&direct, &transaction)
                    .unwrap()
                    .collect::<Vec<_>>()
            );
            if step % 2 == 0 {
                transaction.commit();
            }
        }
        if step % 2 != 0 {
            assert_eq!(assignment, original);
        }
    }
}
#[test]
fn sparse_swaps_solve_identically_in_list_and_mixed_scoring_backends() {
    let rows: Vec<_> = (0..40)
        .map(|e| {
            ListEntity::new(vec![2 * e + 1, 2 * e]).with_value_range(
                [e, (e + 1) % 40, (e + 39) % 40]
                    .into_iter()
                    .flat_map(|g| [2 * g, 2 * g + 1])
                    .collect(),
            )
        })
        .collect();
    let indexed = ListProblem::new_indexed((0..80).rev().collect(), rows.clone()).unwrap();
    let direct = ListProblem::new((0..80).rev().collect(), rows).unwrap();
    let score = |values: &[usize]| SimpleScore::new(i64::from(values[0] < values[1]));
    for neighborhood in [Neighborhood::Swap, Neighborhood::UniqueSwap] {
        let config = LocalSearchConfig {
            neighborhood,
            max_steps: 3,
            max_evaluations: 10000,
            assert_scores: true,
            ..Default::default()
        };
        let mut results = vec![];
        for p in [&direct, &indexed] {
            let mut network = ListConstraintNetwork::new(40, 80);
            for e in 0..40 {
                network
                    .add_constraint(
                        format!("row{e}"),
                        vec![ListScope::Entity(e)],
                        move |facts| match &facts[0] {
                            ListFact::Entity(row) => score(row),
                            _ => unreachable!(),
                        },
                    )
                    .unwrap();
            }
            let result =
                list_local_search_from(p, &network, p.initial_assignment(), config.clone())
                    .unwrap();
            results.push((
                result.best.as_ref().unwrap().assignment.lists().to_vec(),
                result.best.unwrap().score,
                result.completed_steps,
            ));
            let mut builder = StreamGraphBuilder::new();
            let sources = ListStreamSources::<usize>::new(&mut builder);
            builder
                .score("rows", sources.entities.port(), |(_, row)| score(row))
                .unwrap();
            let result = list_stream_local_search_from(
                p,
                builder.build(),
                sources,
                p.initial_assignment(),
                config.clone(),
            )
            .unwrap();
            results.push((
                result.best.as_ref().unwrap().assignment.lists().to_vec(),
                result.best.unwrap().score,
                result.completed_steps,
            ));
            let mixed = MixedProblem::new(FiniteProblem::<usize>::new(vec![]).unwrap(), p.clone());
            let mut network = MixedConstraintNetwork::new(0, 40, 80);
            for e in 0..40 {
                network
                    .add_constraint(
                        format!("row{e}"),
                        vec![MixedScope::ListEntity(e)],
                        move |facts| match &facts[0] {
                            MixedFact::ListEntity(row) => score(row),
                            _ => unreachable!(),
                        },
                    )
                    .unwrap();
            }
            let configuration = MixedLocalSearchConfig {
                search: config.clone(),
                joint_moves: true,
            };
            let result = mixed_local_search_from(
                &mixed,
                &network,
                mixed.initial_assignment(),
                configuration.clone(),
            )
            .unwrap();
            results.push((
                result
                    .best
                    .as_ref()
                    .unwrap()
                    .assignment
                    .lists()
                    .lists()
                    .to_vec(),
                result.best.unwrap().score,
                result.completed_steps,
            ));
            let mut builder = StreamGraphBuilder::new();
            let sources = MixedStreamSources::<usize, usize>::new(&mut builder);
            builder
                .score("rows", sources.lists.entities.port(), |(_, row)| score(row))
                .unwrap();
            let result = mixed_stream_local_search_from(
                &mixed,
                builder.build(),
                sources,
                mixed.initial_assignment(),
                configuration,
            )
            .unwrap();
            results.push((
                result
                    .best
                    .as_ref()
                    .unwrap()
                    .assignment
                    .lists()
                    .lists()
                    .to_vec(),
                result.best.unwrap().score,
                result.completed_steps,
            ));
        }
        assert!(results.iter().all(|r| r == &results[0]));
        let independently_scored = results[0].0.iter().filter(|row| row[0] < row[1]).count() as i64;
        assert_eq!(results[0].1, SimpleScore::new(independently_scored));
        assert!(independently_scored > 0);
    }
}
