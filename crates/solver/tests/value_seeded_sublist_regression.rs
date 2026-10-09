// Original native regressions; no upstream method credit.
use std::{cell::Cell, cmp::Ordering, rc::Rc};
use timetuck::*;
#[test]
fn every_seed_and_rank_matches_independent_spans_with_pins_and_bounds() {
    let p = ListProblem::new(
        (0..8).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3]).with_pinned_prefix(1),
            ListEntity::new(vec![4, 5]).with_pinned(true),
            ListEntity::new(vec![6]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    for minimum in 1..=4 {
        for maximum in minimum..=5 {
            let index = ValueSeededSubListSelection::new(
                &p,
                &a,
                SubListSize::new(minimum, maximum).unwrap(),
            )
            .unwrap();
            let mut eligible = 0;
            for value in 0..=8 {
                let mut expected = vec![];
                if let Some(pos) = a.location(&value) {
                    let meta = &p.entities()[pos.entity];
                    if !meta.is_pinned() && pos.index >= meta.pinned_prefix() {
                        for length in (minimum..=maximum).rev() {
                            for start in meta.pinned_prefix()..a.lists()[pos.entity].len() {
                                if start + length <= a.lists()[pos.entity].len() {
                                    expected.push(ListRange {
                                        entity: pos.entity,
                                        start,
                                        end: start + length,
                                    });
                                }
                            }
                        }
                    }
                }
                if expected.is_empty() {
                    assert_eq!(
                        index.sample_for_with(&value, |_| panic!("ineligible draw")),
                        None
                    );
                } else {
                    eligible += 1;
                    for (rank, span) in expected.iter().enumerate() {
                        assert_eq!(
                            index.sample_for_with(&value, |bound| {
                                assert_eq!(bound, expected.len() as u128);
                                rank as u128
                            }),
                            Some(*span)
                        );
                    }
                }
            }
            assert_eq!(index.seed_count(), eligible);
            assert_eq!(index.is_empty(), eligible == 0);
        }
    }
}
#[test]
fn lazy_consumption_counts_rejections_preserves_repeats_and_fuses() {
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![ListEntity::new(vec![0, 1]).with_pinned_prefix(1)],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let index = ValueSeededSubListSelection::new(&p, &a, SubListSize::new(1, 2).unwrap()).unwrap();
    let reads = Cell::new(0);
    let draws = Cell::new(0);
    let seeds = [0, 2, 99, 1, 1, 1];
    let input = seeds.iter().inspect(|_| reads.set(reads.get() + 1));
    let mut samples = index.random_iter_with(input, 5, |bound| {
        draws.set(draws.get() + 1);
        bound - 1
    });
    assert_eq!((reads.get(), draws.get()), (0, 0));
    let first = samples.next().unwrap();
    assert_eq!(samples.next(), Some(first));
    assert_eq!(samples.next(), None);
    assert_eq!((reads.get(), draws.get()), (5, 2));
    let mut calls = 0;
    let seeds = std::iter::from_fn(|| {
        calls += 1;
        if calls == 2 { None } else { Some(&1) }
    });
    let mut samples = index.random_iter(seeds, 10, 0);
    assert!(samples.next().is_some());
    assert_eq!(samples.next(), None);
    assert_eq!(samples.next(), None);
    drop(samples);
    assert_eq!(calls, 2);
    let empty = ValueSeededSubListSelection::new(&p, &a, SubListSize::new(2, 2).unwrap()).unwrap();
    let mut samples =
        empty.random_iter_with(std::iter::repeat_with(|| panic!("empty read")), 10, |_| {
            panic!("empty draw")
        });
    assert_eq!(samples.next(), None);
    let replay = || {
        index
            .random_iter(std::iter::repeat(&1), 20, 7)
            .collect::<Vec<_>>()
    };
    assert_eq!(replay(), replay());
    assert!(std::panic::catch_unwind(|| index.sample_for_with(&1, |bound| bound)).is_err());
}
#[derive(Debug)]
struct Counted {
    id: usize,
    clones: Rc<Cell<usize>>,
    comparisons: Rc<Cell<usize>>,
}
impl Clone for Counted {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            clones: self.clones.clone(),
            comparisons: self.comparisons.clone(),
        }
    }
}
impl PartialEq for Counted {
    fn eq(&self, other: &Self) -> bool {
        self.comparisons.set(self.comparisons.get() + 1);
        self.id == other.id
    }
}
impl Eq for Counted {}
impl PartialOrd for Counted {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Counted {
    fn cmp(&self, other: &Self) -> Ordering {
        self.comparisons.set(self.comparisons.get() + 1);
        self.id.cmp(&other.id)
    }
}
#[test]
fn indexed_seed_queries_borrow_payloads_and_avoid_linear_owner_search() {
    let clones = Rc::new(Cell::new(0));
    let comparisons = Rc::new(Cell::new(0));
    let values: Vec<_> = (0..2048)
        .map(|id| Counted {
            id,
            clones: clones.clone(),
            comparisons: comparisons.clone(),
        })
        .collect();
    let p = ListProblem::new(values.clone(), vec![ListEntity::new(values)]).unwrap();
    let a = p.initial_assignment();
    clones.set(0);
    let index = ValueSeededSubListSelection::new(&p, &a, SubListSize::new(2, 4).unwrap()).unwrap();
    assert_eq!(clones.get(), 0);
    comparisons.set(0);
    for id in (0..2048).step_by(31) {
        let span = index.sample_for_with(&p.values()[id], |_| 0).unwrap();
        assert_eq!(span.entity, 0);
        assert_eq!(span.end - span.start, 4);
    }
    assert_eq!(clones.get(), 0);
    assert!(
        comparisons.get() < 64 * 67,
        "{} comparisons",
        comparisons.get()
    );
}

#[test]
fn rebuilding_during_moves_and_after_undo_follows_current_owners() {
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![ListEntity::new(vec![0, 1, 2]), ListEntity::new(vec![])],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let size = SubListSize::new(1, 2).unwrap();
    {
        let index = ValueSeededSubListSelection::new(&p, &a, size).unwrap();
        let replay = || {
            index
                .random_iter(std::iter::repeat(&1), 64, 7)
                .collect::<Vec<_>>()
        };
        let expected = replay();
        assert_eq!(expected, replay());
        assert!(expected.iter().all(|span| span.entity == 0));
        assert!(expected.iter().any(|span| *span != expected[0]));
    }
    {
        let moved = p
            .apply(
                &mut a,
                &ListMove::Change {
                    source: ListPosition {
                        entity: 0,
                        index: 1,
                    },
                    destination: ListPosition {
                        entity: 1,
                        index: 0,
                    },
                },
            )
            .unwrap();
        let index = ValueSeededSubListSelection::new(&p, &moved, size).unwrap();
        assert_eq!(
            index.sample_for_with(&1, |_| 0),
            Some(ListRange {
                entity: 1,
                start: 0,
                end: 1
            })
        );
        assert_eq!(
            index.sample_for_with(&2, |_| 0),
            Some(ListRange {
                entity: 0,
                start: 0,
                end: 2
            })
        );
    }
    let index = ValueSeededSubListSelection::new(&p, &a, size).unwrap();
    assert_eq!(
        index.sample_for_with(&1, |_| 0),
        Some(ListRange {
            entity: 0,
            start: 0,
            end: 2
        })
    );
    let bad = ListAssignment::new(vec![vec![0, 0], vec![1, 2]]);
    assert!(ValueSeededSubListSelection::new(&p, &bad, size).is_err());
}
