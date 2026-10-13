//! Phase-local owned position plans; shuffle handles and materialize selected moves.
use crate::list_k_opt::PositionPlan;
use crate::{ListAssignment, ListKOptConfig, ListMove, ListProblem};
use std::cell::RefCell;
#[derive(Eq, PartialEq)]
struct Shape {
    config: ListKOptConfig,
    entities: Vec<(usize, usize, usize, bool)>,
}
struct Prefix {
    shape: Option<Shape>,
    moves: Vec<PositionPlan>,
    generator: Option<Box<dyn Iterator<Item = PositionPlan>>>,
    exhausted: bool,
    #[cfg(test)]
    generated: usize,
    #[cfg(test)]
    materialized: std::cell::Cell<usize>,
}
enum Handle {
    Cached(usize),
    Owned(PositionPlan),
}
pub(crate) struct KOptWindowCache(RefCell<Prefix>);
impl KOptWindowCache {
    pub(crate) fn new() -> Self {
        Self(RefCell::new(Prefix {
            shape: None,
            moves: Vec::new(),
            generator: None,
            exhausted: false,
            #[cfg(test)]
            generated: 0,
            #[cfg(test)]
            materialized: std::cell::Cell::new(0),
        }))
    }
    fn tokens<'a, V: Eq + Clone>(
        &'a self,
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
        config: ListKOptConfig,
        order: &[usize],
        window: usize,
    ) -> Box<dyn Iterator<Item = Handle> + 'a> {
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
        let tail_shape = shape.entities.clone();
        {
            let mut prefix = self.0.borrow_mut();
            if prefix.shape.as_ref() != Some(&shape) {
                prefix.generator = Some(Box::new(crate::list_k_opt::ordered_positions(
                    config,
                    shape.entities.clone().into_iter(),
                )));
                prefix.shape = Some(shape);
                prefix.moves.clear();
                prefix.exhausted = false;
            }
        }
        let limit = window.saturating_mul(2).min(8192);
        let mut index = 0;
        let mut tail = None;
        Box::new(std::iter::from_fn(move || {
            let mut prefix = self.0.borrow_mut();
            if index < prefix.moves.len() {
                let token = Handle::Cached(index);
                index += 1;
                return Some(token);
            }
            if prefix.exhausted {
                return None;
            }
            let next = if index < limit {
                prefix
                    .generator
                    .as_mut()
                    .expect("owned shape generator")
                    .next()
            } else {
                tail.get_or_insert_with(|| {
                    crate::list_k_opt::ordered_positions(config, tail_shape.clone().into_iter())
                        .skip(index)
                })
                .next()
            };
            #[cfg(test)]
            if next.is_some() {
                prefix.generated += 1;
            }
            if index < limit {
                match next {
                    Some(plan) => {
                        prefix.moves.push(plan);
                        let token = Handle::Cached(index);
                        index += 1;
                        Some(token)
                    }
                    None => {
                        prefix.exhausted = true;
                        None
                    }
                }
            } else {
                index += usize::from(next.is_some());
                next.map(Handle::Owned)
            }
        }))
    }
    fn materialize<V>(&self, handle: Handle) -> ListMove<V> {
        let prefix = self.0.borrow();
        #[cfg(test)]
        prefix.materialized.set(prefix.materialized.get() + 1);
        match handle {
            Handle::Cached(index) => prefix.moves[index].materialize(),
            Handle::Owned(plan) => plan.materialize(),
        }
    }
    pub(crate) fn shuffled<'a, V: Eq + Clone>(
        &'a self,
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
        config: ListKOptConfig,
        order: &[usize],
        ordering: (
            crate::MoveOrder,
            Option<&'a mut crate::random::SplitMix64>,
            u64,
        ),
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a> {
        let (mode, random, remaining) = ordering;
        let crate::MoveOrder::ShuffledMoves { window, .. } = mode else {
            unreachable!("cached window ordering")
        };
        let tokens = self.tokens(problem, assignment, config, order, window.get());
        Box::new(
            crate::candidate_window::order_moves(tokens, mode, random, remaining)
                .map(move |handle| self.materialize(handle)),
        )
    }
    #[cfg(test)]
    fn ordered<'a, V: Eq + Clone>(
        &'a self,
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
        config: ListKOptConfig,
        order: &[usize],
        window: usize,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a> {
        Box::new(
            self.tokens(problem, assignment, config, order, window)
                .map(move |token| self.materialize(token)),
        )
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

#[cfg(test)]
mod handle_tests {
    use super::*;
    use crate::{ListEntity, MoveOrder, random::SplitMix64};
    use std::num::NonZeroUsize;
    #[test]
    fn lookahead_materializes_only_selected_moves_and_preserves_the_seeded_sequence() {
        let p = ListProblem::new(
            (0..12).collect(),
            vec![ListEntity::new((0..12).collect()).with_pinned_prefix(1)],
        )
        .unwrap();
        let a = p.initial_assignment();
        let k = ListKOptConfig::new(2, 4).unwrap();
        let cache = KOptWindowCache::new();
        for window in [1, 7, 4096] {
            for seed in 0..3 {
                let mode = MoveOrder::ShuffledMoves {
                    seed,
                    window: NonZeroUsize::new(window).unwrap(),
                };
                let before = cache.0.borrow().materialized.get();
                let mut random = SplitMix64::new(seed);
                let actual: Vec<_> = cache
                    .shuffled(&p, &a, k, &[0], (mode, Some(&mut random), 10000))
                    .take(3)
                    .collect();
                assert_eq!(cache.0.borrow().materialized.get() - before, 3);
                let mut random = SplitMix64::new(seed);
                let expected: Vec<_> = crate::candidate_window::order_moves(
                    crate::list_k_opt::ordered(&p, &a, k, [0].into_iter()),
                    mode,
                    Some(&mut random),
                    10000,
                )
                .take(3)
                .collect();
                assert_eq!(actual, expected);
                assert!(cache.0.borrow().moves.len() <= 8192);
            }
        }
    }
    #[test]
    fn grown_prefix_and_uncached_tail_preserve_original_moves_across_restarts() {
        let p =
            ListProblem::new((0..10).collect(), vec![ListEntity::new((0..10).collect())]).unwrap();
        let a = p.initial_assignment();
        let k = ListKOptConfig::new(2, 4).unwrap();
        let cache = KOptWindowCache::new();
        for (window, count) in [(2, 2), (7, 10), (7, 31), (16, 43), (2, 51)] {
            let seed = 9;
            let mode = MoveOrder::ShuffledMoves {
                seed,
                window: NonZeroUsize::new(window).unwrap(),
            };
            let mut actual_random = SplitMix64::new(seed);
            let mut expected_random = SplitMix64::new(seed);
            let actual: Vec<_> = cache
                .shuffled(&p, &a, k, &[0], (mode, Some(&mut actual_random), 10000))
                .take(count)
                .collect();
            let expected: Vec<_> = crate::candidate_window::order_moves(
                crate::list_k_opt::ordered(&p, &a, k, [0].into_iter()),
                mode,
                Some(&mut expected_random),
                10000,
            )
            .take(count)
            .collect();
            assert_eq!(actual, expected);
            assert!(cache.0.borrow().moves.len() <= 32);
        }
    }
}
