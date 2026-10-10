// Original native regressions; no upstream method credit or Java PRNG parity.
use std::{cell::Cell, num::NonZeroUsize, rc::Rc};
use timetuck::*;
fn provider(left: Vec<i32>, right: Vec<i32>) -> SampledListValueSwapProvider<i32> {
    SampledListValueSwapProvider {
        left,
        right,
        include_noops: false,
        seed: u64::MAX,
        attempts_per_step: 100,
    }
}
#[test]
fn every_small_partial_state_matches_conditional_sampling_and_replays() {
    for code in 0..27 {
        let mut rows = vec![vec![], vec![]];
        let mut state = code;
        for value in 0..3 {
            let owner = state % 3;
            state /= 3;
            if owner < 2 {
                rows[owner].push(value);
            }
        }
        let p = ListProblem::new(
            vec![0, 1, 2],
            rows.into_iter().map(ListEntity::new).collect(),
        )
        .unwrap()
        .allowing_unassigned();
        let a = p.initial_assignment();
        let c = provider(vec![2, 0, 0, 1], vec![1, 2, 2, 0]);
        for step in [0, 1, 17] {
            let mut rng = SampleRandom::new(c.seed.wrapping_add(step));
            let mut expected = vec![];
            for _ in 0..c.attempts_per_step {
                let left = c.left[rng.index(NonZeroUsize::new(c.left.len()).unwrap())];
                let candidates: Vec<_> = list_value_swap_moves(
                    &p,
                    &a,
                    &[left],
                    &c.right,
                    ListSwapPairing::Cartesian,
                    false,
                )
                .unwrap()
                .collect();
                if !candidates.is_empty() {
                    let i = if candidates.len() == 1 {
                        0
                    } else {
                        rng.index(NonZeroUsize::new(candidates.len()).unwrap())
                    };
                    expected.push(candidates[i].clone());
                }
            }
            let actual: Vec<_> = c.select(&p, &a, step).unwrap().collect();
            assert_eq!(actual, expected, "state {code} step {step}");
            assert_eq!(
                c.select(&p, &a, step).unwrap().next(),
                actual.first().cloned()
            );
            assert_eq!(c.select(&p, &a, step).unwrap().collect::<Vec<_>>(), actual);
            if let Some(m) = actual.first() {
                let mut changed = a.clone();
                p.apply(&mut changed, m).unwrap().commit();
                for m in c.select(&p, &changed, step + 1).unwrap() {
                    let mut trial = changed.clone();
                    drop(p.apply(&mut trial, &m).unwrap());
                    assert_eq!(trial, changed);
                }
            }
        }
    }
}
#[test]
fn filters_pins_ranges_unassignment_and_dependencies_with_bounded_empty_results() {
    let p = ListProblem::from_entity_ranges(vec![
        ListEntity::new(vec![0, 1])
            .with_pinned_prefix(1)
            .with_value_range(vec![0, 1, 2, 4]),
        ListEntity::new(vec![2]).with_value_range(vec![1, 2]),
        ListEntity::new(vec![3])
            .with_pinned(true)
            .with_value_range(vec![3]),
    ])
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let c = provider(vec![0, 1, 3], vec![0, 2, 3, 4]);
    let all: Vec<_> =
        list_value_swap_moves(&p, &a, &c.left, &c.right, ListSwapPairing::Cartesian, false)
            .unwrap()
            .collect();
    let actual: Vec<_> = c.select(&p, &a, 0).unwrap().collect();
    assert_eq!(actual.len(), 100);
    assert_eq!(all.len(), 2);
    assert!(all.iter().all(|m| actual.contains(m)));
    assert!(actual.iter().all(|m| all.contains(m)));
    assert!(provider(vec![99], vec![]).select(&p, &a, 0).is_err());
    assert!(c.select(&p, &ListAssignment::new(vec![]), 0).is_err());
    for c in [
        provider(vec![], vec![1]),
        provider(vec![0, 3], vec![1]),
        provider(vec![1], vec![]),
        SampledListValueSwapProvider {
            attempts_per_step: 0,
            ..c.clone()
        },
    ] {
        assert_eq!(c.select(&p, &a, 0).unwrap().count(), 0);
    }
    let required = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1])]).unwrap();
    assert_eq!(
        provider(vec![2], vec![1])
            .select(&required, &required.initial_assignment(), 0)
            .unwrap()
            .count(),
        0
    );
    let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0, 1, 2])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0], vec![1]])
        .unwrap();
    let a = p.initial_assignment();
    let mut c = provider(vec![0, 1, 2], vec![0, 1, 2]);
    assert_eq!(c.select(&p, &a, 0).unwrap().count(), 0);
    c.include_noops = true;
    let result: Vec<_> = c.select(&p, &a, 0).unwrap().collect();
    assert!(!result.is_empty() && result.len() < 100);
    assert!(result.iter().all(|m| *m == ListMove::Composite(vec![])));
}
fn reward(row: &[i32]) -> SimpleScore {
    SimpleScore::new(i64::from(row == [7, 3]))
}
#[test]
fn provider_improves_list_and_mixed_search_with_both_backends() {
    let p = ListProblem::new(vec![3, 7], vec![ListEntity::new(vec![3, 7])]).unwrap();
    let mut n = ListConstraintNetwork::new(1, 2);
    n.add_constraint("order", vec![ListScope::Entity(0)], |f| {
        let ListFact::Entity(row) = f[0] else {
            unreachable!()
        };
        reward(row)
    })
    .unwrap();
    let config = LocalSearchConfig {
        max_steps: 2,
        assert_scores: true,
        ..Default::default()
    };
    for stream in [false, true] {
        let mut c = provider(vec![3, 7], vec![7, 3]);
        let r = if stream {
            let mut b = StreamGraphBuilder::new();
            let sources = ListStreamSources::new(&mut b);
            b.score("order", sources.entities.port(), |(_, row)| reward(row))
                .unwrap();
            list_stream_local_search_from_with_provider(
                &p,
                b.build(),
                sources,
                p.initial_assignment(),
                config,
                &mut c,
            )
        } else {
            list_local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut c)
        }
        .unwrap();
        assert_eq!(r.best.unwrap().assignment.lists(), &[vec![7, 3]]);
    }
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![8, 9]).with_initial(8)]).unwrap(),
        p,
    );
    let mut n = MixedConstraintNetwork::new(1, 1, 2);
    n.add_constraint("order", vec![MixedScope::ListEntity(0)], |f| {
        let MixedFact::ListEntity(row) = f[0] else {
            unreachable!()
        };
        reward(row)
    })
    .unwrap();
    let config = MixedLocalSearchConfig {
        search: config,
        ..Default::default()
    };
    for stream in [false, true] {
        let mut c = provider(vec![3, 7], vec![7, 3]);
        let r = if stream {
            let mut b = StreamGraphBuilder::new();
            let sources = MixedStreamSources::new(&mut b);
            b.score("order", sources.lists.entities.port(), |(_, row)| {
                reward(row)
            })
            .unwrap();
            mixed_stream_local_search_from_with_provider(
                &p,
                b.build(),
                sources,
                p.initial_assignment(),
                config,
                &mut c,
            )
        } else {
            mixed_local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut c)
        }
        .unwrap();
        let best = r.best.unwrap();
        assert_eq!(best.assignment.lists().lists(), &[vec![7, 3]]);
        assert_eq!(best.assignment.basic().values(), &[Some(8)]);
        for m in c.moves(&p, &best.assignment, 1) {
            let mut trial = best.assignment.clone();
            p.apply(&mut trial, &m).unwrap().commit();
            assert_eq!(trial.lists().lists(), &[vec![3, 7]]);
        }
    }
}

#[derive(Debug)]
struct Counted {
    id: usize,
    clones: Rc<Cell<usize>>,
}
impl Clone for Counted {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Counted {
    fn eq(&self, o: &Self) -> bool {
        self.id == o.id
    }
}
impl Eq for Counted {}
#[test]
fn ordered_and_sampled_selection_validate_without_cloning_values() {
    let clones = Rc::new(Cell::new(0));
    let values: Vec<_> = (0..32)
        .map(|id| Counted {
            id,
            clones: clones.clone(),
        })
        .collect();
    let p = ListProblem::new(
        values.clone(),
        values
            .iter()
            .cloned()
            .map(|v| ListEntity::new(vec![v]))
            .collect(),
    )
    .unwrap();
    let a = p.initial_assignment();
    let left = vec![values[0].clone(); 20];
    let right = vec![values[1].clone()];
    clones.set(0);
    let moves =
        list_value_swap_moves(&p, &a, &left, &right, ListSwapPairing::Cartesian, false).unwrap();
    assert_eq!(clones.get(), 0);
    assert_eq!(moves.count(), 20);
    // Validation borrows values; index-only swaps need no owned proposal payloads.
    assert_eq!(clones.get(), 0);
    let c = SampledListValueSwapProvider {
        left,
        right,
        include_noops: false,
        seed: 1,
        attempts_per_step: 20,
    };
    clones.set(0);
    let moves = c.select(&p, &a, 0).unwrap();
    assert_eq!(clones.get(), 0);
    assert_eq!(moves.count(), 20);
    assert_eq!(clones.get(), 0);
}
