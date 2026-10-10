use timetuck::*;

fn policies() -> [SampledSubListNeighborhood; 2] {
    [
        SampledSubListNeighborhood::Change(SubListChangeConfig::default()),
        SampledSubListNeighborhood::Swap(SubListSwapConfig::asymmetric(1, 2, 2, 3).unwrap()),
    ]
}
fn problem() -> ListProblem<i32> {
    ListProblem::new(
        (1..=6).collect(),
        vec![
            ListEntity::new(vec![1, 2, 3]).with_pinned_prefix(1),
            ListEntity::new(vec![4, 5]),
            ListEntity::new(vec![6]).with_pinned(true),
            ListEntity::new(vec![]).with_value_range(vec![2, 3, 4, 5]),
        ],
    )
    .unwrap()
    .allowing_unassigned()
}
#[test]
fn replay_preserves_sampler_order_and_rebuilds_after_commits() {
    let p = problem();
    let a = p.initial_assignment();
    for policy in policies() {
        let provider = SampledSubListProvider::new(policy, u64::MAX, 300);
        for step in [0, 1, 9] {
            let seed = u64::MAX.wrapping_add(step);
            let proposals: Box<dyn Iterator<Item = ListMove<i32>>> = match policy {
                SampledSubListNeighborhood::Change(c) => {
                    Box::new(sampled_sublist_change_moves(&p, &a, c, seed, 300).unwrap())
                }
                SampledSubListNeighborhood::Swap(c) => {
                    Box::new(sampled_sublist_swap_moves(&p, &a, c, seed, 300).unwrap())
                }
            };
            let expected: Vec<_> = proposals
                .filter(|m| {
                    let mut copy = a.clone();
                    p.apply(&mut copy, m).is_ok_and(|applied| *applied != a)
                })
                .collect();
            let moves: Vec<_> = provider.select(&p, &a, step).unwrap().collect();
            assert!(!moves.is_empty());
            assert_eq!(moves, expected);
            assert!(moves.len() <= 300);
            // Partial consumption cannot advance provider state.
            assert_eq!(
                provider.select(&p, &a, step).unwrap().next(),
                moves.first().cloned()
            );
            assert_eq!(
                provider.select(&p, &a, step).unwrap().collect::<Vec<_>>(),
                moves
            );
            let mut changed = a.clone();
            p.apply(&mut changed, &moves[0]).unwrap().commit();
            assert_ne!(changed, a);
            for m in provider.select(&p, &changed, step + 1).unwrap() {
                let mut trial = changed.clone();
                let applied = p.apply(&mut trial, &m).unwrap();
                assert_ne!(*applied, changed);
                assert_eq!(applied.lists()[0][0], 1);
                assert_eq!(applied.lists()[2], vec![6]);
            }
        }
    }
}
#[test]
fn dependency_failures_and_unchanged_proposals_never_reach_scoring() {
    let p = ListProblem::new(vec![1, 2, 3, 4], vec![ListEntity::new(vec![1, 2, 3, 4])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0], vec![1], vec![2]])
        .unwrap();
    let a = p.initial_assignment();
    for policy in policies() {
        let provider = SampledSubListProvider::new(policy, 1, 1000);
        assert_eq!(provider.select(&p, &a, 0).unwrap().count(), 0);
    }
}
#[test]
fn empty_and_invalid_openings_are_bounded_and_explicit() {
    let p = ListProblem::new(vec![1], vec![ListEntity::new(vec![1]).with_pinned(true)]).unwrap();
    let a = p.initial_assignment();
    for policy in policies() {
        let provider = SampledSubListProvider::new(policy, 1, u64::MAX);
        assert_eq!(provider.select(&p, &a, 0).unwrap().count(), 0);
        assert!(
            provider
                .select(&p, &ListAssignment::new(vec![]), 0)
                .is_err()
        );
        let zero = SampledSubListProvider::new(policy, 1, 0);
        assert_eq!(zero.select(&p, &a, 0).unwrap().count(), 0);
    }
    let bad = SampledSubListProvider::new(
        SampledSubListNeighborhood::Change(SubListChangeConfig::default().with_unassignment(true)),
        1,
        0,
    );
    assert!(matches!(
        bad.select(&p, &a, 0),
        Err(ListError::UnassignmentNotAllowed)
    ));
}
fn plan() -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: 12,
            selection: MoveSelection::BestAccepted,
            assert_scores: true,
            ..Default::default()
        })],
        ..Default::default()
    }
}
#[test]
fn complete_list_and_mixed_plans_improve_with_both_policies() {
    for policy in policies() {
        let p = ListProblem::new(
            vec![1, 2, 3, 4],
            vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3, 4])],
        )
        .unwrap();
        let mut n = ListConstraintNetwork::new(2, 4);
        n.add_constraint(
            "sort",
            vec![ListScope::Entity(0), ListScope::Entity(1)],
            |facts| {
                let ListFact::Entity(_left) = facts[0] else {
                    unreachable!()
                };
                let ListFact::Entity(right) = facts[1] else {
                    unreachable!()
                };
                SimpleScore::new(right.iter().map(|&v| i64::from(v <= 2)).sum::<i64>())
            },
        )
        .unwrap();
        let mut provider = SampledSubListProvider::new(policy, 13, 1000);
        let mut extensions = SolverExtensions::default();
        extensions.move_providers.bind(0, &mut provider);
        let result = solve_list_with_extensions(&p, &n, plan(), extensions).unwrap();
        assert_eq!(result.score, Some(SimpleScore::new(2)));
        assert!(result.complete);
        let mixed = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![7, 8]).with_initial(7)]).unwrap(),
            p,
        );
        let mut n = MixedConstraintNetwork::new(1, 2, 4);
        n.add_constraint(
            "sort",
            vec![MixedScope::ListEntity(0), MixedScope::ListEntity(1)],
            |facts| {
                let MixedFact::ListEntity(_left) = facts[0] else {
                    unreachable!()
                };
                let MixedFact::ListEntity(right) = facts[1] else {
                    unreachable!()
                };
                SimpleScore::new(right.iter().map(|&v| i64::from(v <= 2)).sum::<i64>())
            },
        )
        .unwrap();
        let mut extensions = SolverExtensions::default();
        extensions.move_providers.bind(0, &mut provider);
        let result = solve_mixed_with_extensions(
            &mixed,
            &n,
            MixedSolverSequence {
                phases: vec![MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                    search: LocalSearchConfig {
                        max_steps: 12,
                        selection: MoveSelection::BestAccepted,
                        assert_scores: true,
                        ..Default::default()
                    },
                    ..Default::default()
                })],
                ..Default::default()
            },
            extensions,
        )
        .unwrap();
        assert_eq!(result.score, Some(SimpleScore::new(2)));
        assert!(result.complete);
        assert_eq!(result.assignment.basic().values(), &[Some(7)]);
    }
}

#[derive(Debug)]
struct CountedValue {
    id: i32,
    clones: std::rc::Rc<std::cell::Cell<usize>>,
}
impl Clone for CountedValue {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for CountedValue {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for CountedValue {}

#[test]
fn borrowed_validation_preserves_proposals_without_cloning_values() {
    let clones = std::rc::Rc::new(std::cell::Cell::new(0));
    let values = (0..16)
        .map(|id| CountedValue {
            id,
            clones: clones.clone(),
        })
        .collect::<Vec<_>>();
    let p = ListProblem::new(
        values.clone(),
        vec![
            ListEntity::new(values[..8].to_vec()),
            ListEntity::new(values[8..].to_vec()),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    for policy in policies() {
        let provider = SampledSubListProvider::new(policy, 43, 128);
        let candidates: Vec<ListMove<CountedValue>> = match policy {
            SampledSubListNeighborhood::Change(c) => {
                sampled_sublist_change_moves(&p, &a, c, 43, 128)
                    .unwrap()
                    .collect()
            }
            SampledSubListNeighborhood::Swap(c) => sampled_sublist_swap_moves(&p, &a, c, 43, 128)
                .unwrap()
                .collect(),
        };
        assert!(candidates.len() > 1);
        clones.set(0);
        let expected = candidates
            .iter()
            .filter(|m| {
                let mut fresh = a.clone();
                p.apply(&mut fresh, m).is_ok_and(|trial| *trial != a)
            })
            .cloned()
            .collect::<Vec<_>>();
        assert!(clones.get() > 0);
        clones.set(0);
        let cursor = provider.select(&p, &a, 0).unwrap();
        assert_eq!(clones.get(), 0, "opening does not clone assignment values");
        drop(cursor);
        assert_eq!(clones.get(), 0, "an unused traversal does not clone values");
        let actual = provider.select(&p, &a, 0).unwrap().collect::<Vec<_>>();
        let borrowed_clones = clones.get();
        assert_eq!(actual, expected);
        assert_eq!(borrowed_clones, 0);
        clones.set(0);
        let empty = SampledSubListProvider {
            attempts_per_step: 0,
            ..provider
        };
        assert_eq!(empty.select(&p, &a, 0).unwrap().count(), 0);
        assert_eq!(clones.get(), 0);
    }
}
