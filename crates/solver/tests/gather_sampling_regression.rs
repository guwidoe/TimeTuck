//! Original native sampling tests; no upstream method credit.
use std::{cell::Cell, collections::BTreeSet, rc::Rc};
use timetuck::*;
fn config(min: usize, max: usize, attempts: u64) -> GatherSamplingConfig {
    GatherSamplingConfig::new(min, max, attempts, 93).unwrap()
}

#[test]
fn sampled_successors_cover_small_independent_gather_oracle() {
    for optional in [false, true] {
        let entities = vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![2])];
        let p = ListProblem::new(vec![0, 1, 2], entities).unwrap();
        let p = if optional { p.allowing_unassigned() } else { p };
        let a = p.initial_assignment();
        let mut expected = BTreeSet::new();
        for first in 0..3 {
            for second in [None, Some(0), Some(1), Some(2)] {
                if second == Some(first) {
                    continue;
                }
                let members: Vec<_> = std::iter::once(first).chain(second).collect();
                let destinations = (0..2)
                    .flat_map(|e| (0..=a.lists()[e].len()).map(move |i| Some((e, i))))
                    .chain(optional.then_some(None));
                for destination in destinations {
                    let mut rows: Vec<Vec<_>> = a
                        .lists()
                        .iter()
                        .map(|row| {
                            row.iter()
                                .copied()
                                .filter(|v| !members.contains(v))
                                .collect()
                        })
                        .collect();
                    if let Some((e, i)) = destination {
                        let at = a.lists()[e][..i]
                            .iter()
                            .filter(|v| !members.contains(v))
                            .count();
                        rows[e].splice(at..at, members.iter().copied());
                    }
                    if rows != a.lists() {
                        expected.insert(rows);
                    }
                }
            }
        }
        let mut cfg = config(1, 2, 10_000);
        cfg.unassignment = optional;
        let mut actual = BTreeSet::new();
        let mut count = 0;
        for movement in sampled_list_gather_moves(&p, &a, cfg).unwrap() {
            let mut next = a.clone();
            p.apply(&mut next, &movement).unwrap().commit();
            assert_ne!(next, a);
            actual.insert(next.lists().to_vec());
            count += 1;
        }
        assert!(count <= cfg.attempts);
        assert_eq!(actual, expected);
    }
}

#[test]
fn pins_ranges_partial_assignments_and_dependencies_restrict_samples() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3, 4],
        vec![
            ListEntity::new(vec![0, 1])
                .with_pinned_prefix(1)
                .with_value_range(vec![0, 1]),
            ListEntity::new(vec![2]).with_value_range(vec![1, 2, 4]),
            ListEntity::new(vec![3]).with_pinned(true),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let mut inserted = false;
    for m in sampled_list_gather_moves(&p, &a, config(1, usize::MAX, 500)).unwrap() {
        let mut next = a.clone();
        p.apply(&mut next, &m).unwrap().commit();
        assert_eq!(next.lists()[0][0], 0);
        assert_eq!(next.lists()[2], vec![3]);
        assert!(next.lists()[0].iter().all(|v| [0, 1].contains(v)));
        assert!(next.lists()[1].iter().all(|v| [1, 2, 4].contains(v)));
        if next.location(&4).is_some() {
            inserted = true;
        }
        assert_eq!(
            next.lists().iter().flatten().filter(|v| **v != 4).count(),
            4
        );
    }
    assert!(inserted);
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0]])
        .unwrap();
    let a = p.initial_assignment();
    assert_eq!(
        sampled_list_gather_moves(&p, &a, config(1, 2, 200))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn empty_invalid_and_impossible_inputs_terminate_and_seed_advances_by_step() {
    assert!(GatherSamplingConfig::new(0, 3, 10, 0).is_err());
    assert!(GatherSamplingConfig::new(3, 2, 10, 0).is_err());
    let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0, 1, 2])]).unwrap();
    let a = p.initial_assignment();
    for cfg in [config(1, 2, 0), config(4, usize::MAX, 10)] {
        let mut iter = sampled_list_gather_moves(&p, &a, cfg).unwrap();
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }
    let mut bad = config(1, 1, 10);
    bad.unassignment = true;
    assert!(sampled_list_gather_moves(&p, &a, bad).is_err());
    assert!(
        sampled_list_gather_moves(&p, &ListAssignment::new(vec![vec![99]]), config(1, 2, 10))
            .is_err()
    );
    let empty = ListProblem::<i32>::new(vec![], vec![]).unwrap();
    assert_eq!(
        sampled_list_gather_moves(&empty, &empty.initial_assignment(), config(1, 2, 10))
            .unwrap()
            .count(),
        0
    );
    let singleton = ListProblem::new(vec![0], vec![ListEntity::new(vec![0])]).unwrap();
    assert_eq!(
        sampled_list_gather_moves(
            &singleton,
            &singleton.initial_assignment(),
            config(1, 1, 100)
        )
        .unwrap()
        .count(),
        0
    );
    let pinned =
        ListProblem::new(vec![0], vec![ListEntity::new(vec![0]).with_pinned(true)]).unwrap();
    assert_eq!(
        sampled_list_gather_moves(&pinned, &pinned.initial_assignment(), config(1, 1, 100))
            .unwrap()
            .count(),
        0
    );
    let cfg = config(1, 3, 30);
    let mut provider = GatherSamplingProvider::new(cfg);
    for step in [0, 1, u64::MAX] {
        let actual: Vec<_> = provider.moves(&p, &a, step).collect();
        let mut expected_config = cfg;
        expected_config.seed = cfg.seed.wrapping_add(step);
        let expected: Vec<_> = sampled_list_gather_moves(&p, &a, expected_config)
            .unwrap()
            .collect();
        assert_eq!(actual, expected);
    }
}

#[derive(Debug)]
struct Counted(usize, Rc<Cell<usize>>);
impl Clone for Counted {
    fn clone(&self) -> Self {
        self.1.set(self.1.get() + 1);
        Self(self.0, self.1.clone())
    }
}
impl PartialEq for Counted {
    fn eq(&self, b: &Self) -> bool {
        self.0 == b.0
    }
}
impl Eq for Counted {}
#[test]
fn opening_does_not_clone_values_or_enumerate_groups_and_search_obeys_limits() {
    let clones = Rc::new(Cell::new(0));
    let values: Vec<_> = (0..30).map(|i| Counted(i, clones.clone())).collect();
    let p = ListProblem::new(values.clone(), vec![ListEntity::new(values)]).unwrap();
    let a = p.initial_assignment();
    clones.set(0);
    let mut moves = sampled_list_gather_moves(&p, &a, config(1, usize::MAX, 100)).unwrap();
    assert_eq!(clones.get(), 0);
    assert!(moves.next().is_some());
    let after = clones.get();
    assert!(after > 0);
    drop(moves);
    assert_eq!(after, clones.get());
    let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0, 1, 2])]).unwrap();
    let network = ListConstraintNetwork::<_, SimpleScore>::new(1, 3);
    let mut provider = GatherSamplingProvider::new(config(1, 3, 100));
    let result = list_local_search_from_with_provider(
        &p,
        &network,
        p.initial_assignment(),
        LocalSearchConfig {
            max_steps: 10,
            max_evaluations: 2,
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap();
    assert!(result.evaluations <= 2);
}
