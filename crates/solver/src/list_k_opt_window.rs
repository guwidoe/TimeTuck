//! Phase-local reuse of position-only K-opt candidates before window shuffling.
use crate::{ListAssignment, ListKOptConfig, ListMove, ListProblem};
use std::cell::RefCell;

#[derive(Eq, PartialEq)]
struct Shape {
    config: ListKOptConfig,
    entities: Vec<(usize, usize, usize, bool)>,
}

struct Prefix<V> {
    shape: Option<Shape>,
    moves: Vec<ListMove<V>>,
    exhausted: bool,
    #[cfg(test)]
    generated: usize,
}

pub(crate) struct KOptWindowCache<V>(RefCell<Prefix<V>>);

impl<V: Eq + Clone> KOptWindowCache<V> {
    pub(crate) fn new() -> Self {
        Self(RefCell::new(Prefix {
            shape: None,
            moves: Vec::new(),
            exhausted: false,
            #[cfg(test)]
            generated: 0,
        }))
    }

    /// K-opt produces only positional relocations: payload order, ranges and
    /// scores do not participate in generation. Keep at most one shape and a
    /// bounded prefix per phase, never payload-dependent or cross-phase moves.
    /// Nothing is generated until next(); the original lazy tail stays available.
    pub(crate) fn ordered<'a>(
        &'a self,
        problem: &'a ListProblem<V>,
        assignment: &'a ListAssignment<V>,
        config: ListKOptConfig,
        order: &[usize],
        window: usize,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a> {
        let shape = Shape {
            config,
            entities: order
                .iter()
                .map(|&e| {
                    (
                        e,
                        assignment.lists()[e].len(),
                        problem.entities()[e].pinned_prefix(),
                        problem.entities()[e].is_pinned(),
                    )
                })
                .collect(),
        };
        {
            let mut prefix = self.0.borrow_mut();
            if prefix.shape.as_ref() != Some(&shape) {
                prefix.shape = Some(shape);
                prefix.moves.clear();
                prefix.exhausted = false;
            }
        }
        // A rolling window refills after each score trial. Retain a second
        // window so a short scan does not immediately regenerate the prefix.
        let limit = window.saturating_mul(2).min(8192);
        let order = order.to_vec();
        let mut generated = None;
        let mut index = 0;
        Box::new(std::iter::from_fn(move || {
            let mut prefix = self.0.borrow_mut();
            if let Some(movement) = prefix.moves.get(index) {
                index += 1;
                return Some(movement.clone());
            }
            if prefix.exhausted {
                return None;
            }
            let generator = generated.get_or_insert_with(|| {
                crate::list_k_opt::ordered(problem, assignment, config, order.clone().into_iter())
                    .skip(index)
            });
            let next = generator.next();
            #[cfg(test)]
            if next.is_some() {
                prefix.generated += 1;
            }
            if index < limit {
                if let Some(movement) = &next {
                    prefix.moves.push(movement.clone());
                } else {
                    prefix.exhausted = true;
                }
            }
            index += usize::from(next.is_some());
            next
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ListEntity, MoveOrder, candidate_window::order_moves, random::SplitMix64};
    use std::num::NonZeroUsize;

    #[test]
    fn reordered_payloads_reuse_positions_and_preserve_seeded_windows_and_lazy_tail() {
        let problem = ListProblem::new(
            (0..12).collect(),
            vec![
                ListEntity::new((0..7).collect()).with_pinned_prefix(1),
                ListEntity::new((7..12).collect()),
            ],
        )
        .unwrap();
        let config = ListKOptConfig::new(2, 4).unwrap();
        let cache = KOptWindowCache::new();
        let mut assignment = problem.initial_assignment();
        let order = [1, 0];
        assert!(
            cache
                .ordered(&problem, &assignment, config, &order, 7)
                .next()
                .is_some()
        );
        assert_eq!(cache.0.borrow().generated, 1);
        let expected: Vec<_> =
            crate::list_k_opt::ordered(&problem, &assignment, config, order.into_iter()).collect();
        for seed in 0..4 {
            let movement = expected[seed as usize].clone();
            problem.apply(&mut assignment, &movement).unwrap().commit();
            let fresh: Vec<_> =
                crate::list_k_opt::ordered(&problem, &assignment, config, order.into_iter())
                    .collect();
            assert_eq!(fresh, expected);
            let mut a = SplitMix64::new(seed);
            let mut b = SplitMix64::new(seed);
            let mode = MoveOrder::ShuffledMoves {
                seed,
                window: NonZeroUsize::new(7).unwrap(),
            };
            let actual: Vec<_> = order_moves(
                cache.ordered(&problem, &assignment, config, &order, 7),
                mode,
                Some(&mut a),
                10000,
            )
            .collect();
            assert_eq!(
                actual,
                order_moves(fresh.into_iter(), mode, Some(&mut b), 10000).collect::<Vec<_>>()
            );
            assert_eq!(cache.0.borrow().moves.len(), 14);
            let count = cache.0.borrow().generated;
            assert_eq!(
                cache
                    .ordered(&problem, &assignment, config, &order, 7)
                    .take(7)
                    .collect::<Vec<_>>(),
                expected[..7]
            );
            assert_eq!(cache.0.borrow().generated, count);
        }
    }

    #[test]
    fn changed_shapes_invalidate_and_empty_neighborhoods_remain_lazy() {
        let cache = KOptWindowCache::new();
        for (lengths, prefix, pinned, order, maximum) in [
            ([6, 5], 0, false, [0, 1], 3),
            ([5, 6], 0, false, [0, 1], 3),
            ([5, 6], 1, false, [0, 1], 3),
            ([5, 6], 0, true, [0, 1], 3),
            ([5, 6], 0, true, [1, 0], 3),
            ([5, 6], 0, true, [1, 0], 4),
            ([3, 3], 0, false, [0, 1], 4),
        ] {
            let n = lengths.iter().sum();
            let problem = ListProblem::new(
                (0..n).collect(),
                vec![
                    ListEntity::new((0..lengths[0]).collect())
                        .with_pinned_prefix(prefix)
                        .with_pinned(pinned),
                    ListEntity::new((lengths[0]..n).collect()),
                ],
            )
            .unwrap();
            let assignment = problem.initial_assignment();
            let config = ListKOptConfig::new(2, maximum).unwrap();
            let count = cache.0.borrow().generated;
            let actual = cache.ordered(&problem, &assignment, config, &order, 4096);
            assert_eq!(cache.0.borrow().generated, count);
            let actual: Vec<_> = actual.collect();
            assert_eq!(
                actual,
                crate::list_k_opt::ordered(&problem, &assignment, config, order.into_iter())
                    .collect::<Vec<_>>()
            );
            let count = cache.0.borrow().generated;
            assert_eq!(
                cache
                    .ordered(&problem, &assignment, config, &order, 4096)
                    .collect::<Vec<_>>(),
                actual
            );
            assert_eq!(cache.0.borrow().generated, count);
        }
    }
    #[test]
    fn cached_search_matches_uncached_provider_scores_trials_and_termination() {
        use crate::*;
        struct Original(ListKOptConfig);
        impl MoveProvider<ListProblem<usize>, ListAssignment<usize>, ListMove<usize>> for Original {
            fn moves<'a>(
                &'a mut self,
                p: &'a ListProblem<usize>,
                a: &'a ListAssignment<usize>,
                _: u64,
            ) -> Box<dyn Iterator<Item = ListMove<usize>> + 'a>
            where
                ListMove<usize>: 'a,
            {
                Box::new(list_k_opt_moves(p, a, self.0).unwrap())
            }
        }
        let p = ListProblem::new(
            (0..8).collect(),
            vec![ListEntity::new(vec![0, 5, 2, 7, 1, 6, 3, 4]).with_pinned_prefix(1)],
        )
        .unwrap();
        let k = ListKOptConfig::new(2, 4).unwrap();
        let trace = RefCell::new(Vec::new());
        let mut network = ListConstraintNetwork::new(1, 8);
        network
            .add_constraint("directed order", vec![ListScope::Entity(0)], |facts| {
                let ListFact::Entity(row) = facts[0] else {
                    unreachable!()
                };
                trace.borrow_mut().push(row.to_vec());
                HardSoftScore::new(
                    0,
                    -row.iter()
                        .enumerate()
                        .map(|(i, &v)| ((i + 1) * v) as i64)
                        .sum::<i64>(),
                )
            })
            .unwrap();
        for seed in 0..4 {
            for window in [2, 7, 128] {
                let config = LocalSearchConfig {
                    max_evaluations: 500,
                    max_steps: 6,
                    neighborhood: Neighborhood::KOpt(k),
                    move_order: MoveOrder::ShuffledMoves {
                        seed,
                        window: NonZeroUsize::new(window).unwrap(),
                    },
                    selection: MoveSelection::FirstAccepted,
                    rejected_moves: RejectedMovePolicy::BestScore,
                    ..Default::default()
                };
                trace.borrow_mut().clear();
                let cached =
                    list_local_search_from(&p, &network, p.initial_assignment(), config.clone())
                        .unwrap();
                let trials = trace.replace(Vec::new());
                let reference = list_local_search_from_with_provider(
                    &p,
                    &network,
                    p.initial_assignment(),
                    config,
                    &mut Original(k),
                )
                .unwrap();
                assert_eq!(cached, reference);
                assert_eq!(trials, *trace.borrow());
            }
        }
    }
}
