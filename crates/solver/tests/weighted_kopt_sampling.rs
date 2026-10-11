//! Original exact-weight, snapshot and solver integration regressions.
use std::collections::BTreeSet;
use timetuck::*;

fn sampling(weights: Vec<u64>, attempts: usize) -> ListKOptSampling {
    ListKOptSampling::new(
        ListKOptConfig::new(2, weights.len() + 1).unwrap(),
        41,
        attempts,
    )
    .with_weights(weights)
    .unwrap()
}
fn edges(row: &[usize]) -> BTreeSet<(usize, usize)> {
    (0..row.len())
        .map(|i| {
            let (a, b) = (row[i], row[(i + 1) % row.len()]);
            (a.min(b), a.max(b))
        })
        .collect()
}
fn score(row: &[usize]) -> SimpleScore {
    SimpleScore::new(row.iter().enumerate().map(|(i, &v)| (i * v) as i64).sum())
}
struct WeightedProvider {
    sampling: ListKOptSampling,
    repartition: bool,
    openings: Vec<(u64, ListAssignment<usize>)>,
}
impl MoveProvider<ListProblem<usize>, ListAssignment<usize>, ListMove<usize>> for WeightedProvider {
    fn moves<'a>(
        &'a mut self,
        problem: &'a ListProblem<usize>,
        assignment: &'a ListAssignment<usize>,
        step: u64,
    ) -> Box<dyn Iterator<Item = ListMove<usize>> + 'a>
    where
        ListMove<usize>: 'a,
    {
        self.openings.push((step, assignment.clone()));
        let config = self.sampling.for_step(step);
        if self.repartition {
            Box::new(sample_list_k_opt_repartitions(problem, assignment, config).unwrap())
        } else {
            Box::new(sample_list_k_opt_moves(problem, assignment, config).unwrap())
        }
    }
}
#[test]
fn invalid_weights_and_clone_identity_are_explicit() {
    let base = ListKOptSampling::new(ListKOptConfig::new(2, 4).unwrap(), 7, 20);
    assert_eq!(base.weights(), None);
    assert_eq!(
        base.clone().with_weights(vec![0]),
        Err(ListKOptWeightError::Length {
            expected: 3,
            actual: 1
        })
    );
    assert_eq!(
        base.clone().with_weights(vec![0; 3]),
        Err(ListKOptWeightError::ZeroMass)
    );
    assert_eq!(
        base.clone().with_weights(vec![u64::MAX, 1, 0]),
        Err(ListKOptWeightError::Overflow)
    );
    let config = base.with_weights(vec![u64::MAX - 1, 0, 1]).unwrap();
    let cloned = config.clone();
    let stepped = config.for_step(1);
    assert_eq!(config.weights(), Some([u64::MAX - 1, 0, 1].as_slice()));
    assert_eq!(
        config.weights().unwrap().as_ptr(),
        cloned.weights().unwrap().as_ptr()
    );
    assert_eq!(
        config.weights().unwrap().as_ptr(),
        stepped.weights().unwrap().as_ptr()
    );
    assert_eq!(config, config.for_step(0));
    assert_ne!(config.seed(), stepped.seed());
    assert_eq!(config.attempts(), 20);
}
#[test]
fn unit_weights_preserve_uniform_sequences_for_both_sampler_paths() {
    for n in 0..10 {
        let p = ListProblem::new(
            (0..n).collect(),
            vec![ListEntity::new((0..n).collect()), ListEntity::new(vec![])],
        )
        .unwrap();
        let a = p.initial_assignment();
        for seed in [0, 1, u64::MAX] {
            let uniform = ListKOptSampling::new(ListKOptConfig::new(2, 10).unwrap(), seed, 100);
            let weighted = uniform.clone().with_weights(vec![1; 9]).unwrap();
            assert_eq!(
                sample_list_k_opt_moves(&p, &a, uniform.clone())
                    .unwrap()
                    .collect::<Vec<_>>(),
                sample_list_k_opt_moves(&p, &a, weighted.clone())
                    .unwrap()
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                sample_list_k_opt_repartitions(&p, &a, uniform)
                    .unwrap()
                    .collect::<Vec<_>>(),
                sample_list_k_opt_repartitions(&p, &a, weighted)
                    .unwrap()
                    .collect::<Vec<_>>()
            );
        }
    }
}
#[test]
fn zero_feasible_mass_finishes_without_consuming_attempt_budget() {
    // k=4 is impossible on five values, and k=6 exceeds the snapshot.
    let p = ListProblem::new((0..5).collect(), vec![ListEntity::new((0..5).collect())]).unwrap();
    let a = p.initial_assignment();
    for weights in [vec![0, 0, 9, 0, 0], vec![0, 0, 0, 0, 9]] {
        let config = sampling(weights, usize::MAX);
        assert!(
            sample_list_k_opt_moves(&p, &a, config.clone())
                .unwrap()
                .next()
                .is_none()
        );
        assert!(
            sample_list_k_opt_repartitions(&p, &a, config)
                .unwrap()
                .next()
                .is_none()
        );
    }
    let malformed = ListAssignment::new(vec![]);
    assert!(sample_list_k_opt_moves(&p, &malformed, sampling(vec![1], 0)).is_err());
    assert!(sample_list_k_opt_repartitions(&p, &malformed, sampling(vec![1], 0)).is_err());
}
#[test]
fn one_hot_sizes_preserve_pins_payload_membership_and_atomic_undo() {
    let p = ListProblem::new(
        (0..16).collect(),
        vec![
            ListEntity::new((0..8).collect()).with_pinned_prefix(2),
            ListEntity::new((8..15).collect()).with_pinned(true),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    for k in 2..=6 {
        let mut weights = vec![0; 5];
        weights[k - 2] = 17;
        let config = sampling(weights, 300);
        let moves = sample_list_k_opt_moves(&p, &a, config.clone())
            .unwrap()
            .collect::<Vec<_>>();
        assert!(!moves.is_empty() && moves.len() <= 300);
        for movement in moves {
            let mut next = a.clone();
            {
                let trial = p.apply(&mut next, &movement).unwrap();
                assert_eq!(&trial.lists()[0][..2], &[0, 1]);
                assert_eq!(trial.lists()[1], a.lists()[1]);
                assert!(trial.lists()[2].is_empty());
                assert_eq!(trial.location(&15), None);
                assert_eq!(
                    edges(&a.lists()[0][2..])
                        .difference(&edges(&trial.lists()[0][2..]))
                        .count(),
                    k
                );
            }
            assert_eq!(next, a);
        }
        let moves = sample_list_k_opt_repartitions(&p, &a, config)
            .unwrap()
            .collect::<Vec<_>>();
        assert!(!moves.is_empty() && moves.len() <= 300);
        for movement in moves {
            let mut next = a.clone();
            {
                let trial = p.apply(&mut next, &movement).unwrap();
                let combined = trial.lists()[0][2..]
                    .iter()
                    .chain(&trial.lists()[2])
                    .copied()
                    .collect::<Vec<_>>();
                assert_eq!(
                    edges(&a.lists()[0][2..])
                        .difference(&edges(&combined))
                        .count(),
                    k
                );
                assert_eq!(&trial.lists()[0][..2], &[0, 1]);
                assert_eq!(trial.lists()[1], a.lists()[1]);
                assert_eq!(trial.location(&15), None);
            }
            assert_eq!(next, a);
        }
    }
}
#[test]
fn weighted_search_matches_manual_repeated_openings_for_both_backends() {
    for repartition in [false, true] {
        let p = ListProblem::new(
            (0..8).collect(),
            vec![
                ListEntity::new(vec![0, 7, 6, 5, 4, 3, 2, 1]).with_pinned_prefix(1),
                ListEntity::new(vec![]),
            ],
        )
        .unwrap();
        let sampling = sampling(vec![1, 7, 0, 3, 0, 2], 150);
        let neighborhood = if repartition {
            Neighborhood::RepartitionKOpt(sampling.clone())
        } else {
            Neighborhood::SampledKOpt(sampling.clone())
        };
        let total = |a: &ListAssignment<usize>| score(&a.lists()[0]) + score(&a.lists()[1]);
        let mut manual = p.initial_assignment();
        let mut evaluations = 1;
        let mut steps = 0;
        for step in 0..3 {
            let before = manual.clone();
            let old = total(&before);
            let next_config = if repartition {
                Neighborhood::RepartitionKOpt(sampling.for_step(step))
            } else {
                Neighborhood::SampledKOpt(sampling.for_step(step))
            };
            let mut selected = None;
            for movement in list_moves(&p, &before, next_config).unwrap() {
                evaluations += 1;
                let trial = p.apply(&mut manual, &movement).unwrap();
                if total(&trial) > old {
                    selected = Some(trial.clone());
                    break;
                }
            }
            match selected {
                Some(a) => {
                    manual = a;
                    steps += 1;
                }
                None => break,
            }
        }
        assert!(steps > 0);
        let mut network = ListConstraintNetwork::new(2, 8);
        for entity in 0..2 {
            network
                .add_constraint(
                    format!("rank{entity}"),
                    vec![ListScope::Entity(entity)],
                    |facts| {
                        let ListFact::Entity(row) = facts[0] else {
                            unreachable!()
                        };
                        score(row)
                    },
                )
                .unwrap();
        }
        let config = LocalSearchConfig {
            neighborhood,
            max_evaluations: 10000,
            max_steps: 3,
            selection: MoveSelection::FirstAccepted,
            acceptance: Acceptance::StrictImprovement,
            assert_scores: true,
            ..Default::default()
        };
        let scoped =
            list_local_search_from(&p, &network, p.initial_assignment(), config.clone()).unwrap();
        let mut provider = WeightedProvider {
            sampling,
            repartition,
            openings: vec![],
        };
        let supplied = list_local_search_from_with_provider(
            &p,
            &network,
            p.initial_assignment(),
            config.clone(),
            &mut provider,
        )
        .unwrap();
        assert_eq!(scoped, supplied);
        assert!(!provider.openings.is_empty());
        assert_eq!(provider.openings[0], (0, p.initial_assignment()));
        for (expected, (step, _)) in provider.openings.iter().enumerate() {
            assert_eq!(*step, expected as u64);
        }
        let mut builder = StreamGraphBuilder::new();
        let sources = ListStreamSources::new(&mut builder);
        builder
            .score("rank", sources.entities.port(), |(_, row)| score(row))
            .unwrap();
        let stream = list_stream_local_search_from(
            &p,
            builder.build(),
            sources,
            p.initial_assignment(),
            config,
        )
        .unwrap();
        assert_eq!(scoped, stream);
        assert_eq!(scoped.evaluations, evaluations);
        assert_eq!(scoped.completed_steps, steps);
        assert_eq!(scoped.best.unwrap().assignment, manual);
    }
}
