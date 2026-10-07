use std::{cell::Cell, rc::Rc};
use timetuck::*;
fn pos(entity: usize, index: usize) -> Option<ListPosition> {
    Some(ListPosition { entity, index })
}
#[test]
fn groups_intersect_ranges_and_count_slots_with_pins_and_optional_unassignment() {
    let p = ListProblem::new(
        (0..6).collect(),
        vec![
            ListEntity::new(vec![0, 1])
                .with_pinned_prefix(2)
                .with_value_range(vec![0, 1, 2]),
            ListEntity::new(vec![2, 3])
                .with_pinned_prefix(1)
                .with_value_range(vec![1, 2, 3]),
            ListEntity::new(vec![4]).with_pinned(true),
            ListEntity::new(vec![]).with_value_range(vec![5]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let index = ListDestinationIndex::new(&p, &a, true).unwrap();
    let s = index.for_values(&[1, 2, 1]).unwrap();
    assert_eq!(s.len(), 4);
    assert!(!s.is_empty());
    assert_eq!(
        s.iter().collect::<Vec<_>>(),
        vec![None, pos(0, 2), pos(1, 1), pos(1, 2)]
    );
    assert_eq!(s.get(4), None);
    assert_eq!(s.get(usize::MAX), None);
    assert_eq!(
        s.iter().rev().collect::<Vec<_>>(),
        vec![pos(1, 2), pos(1, 1), pos(0, 2), None]
    );
    let none = index.for_values(&[1, 5]).unwrap();
    assert_eq!(none.iter().collect::<Vec<_>>(), vec![None]);
    assert_eq!(
        index
            .for_values(&[])
            .unwrap()
            .entities()
            .collect::<Vec<_>>(),
        vec![0, 1, 3]
    );
    assert!(matches!(
        index.for_value(&99),
        Err(ListDestinationError::Model(ListError::ValueOutsideDomain))
    ));
}
#[test]
fn uniform_sampling_uses_slots_including_null_and_does_not_advance_for_singletons() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![]), ListEntity::new(vec![1, 2, 3])],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let index = ListDestinationIndex::new(&p, &a, true).unwrap();
    let s = index.for_value(&1).unwrap();
    let mut rng = SampleRandom::new(81);
    let mut counts = vec![0usize; s.len()];
    let choices = s.iter().collect::<Vec<_>>();
    for _ in 0..12000 {
        let x = s.sample(&mut rng).unwrap();
        counts[choices.iter().position(|p| *p == x).unwrap()] += 1;
    }
    assert!(
        counts.iter().all(|&n| (1600..2400).contains(&n)),
        "{counts:?}"
    );
    let p = ListProblem::new(vec![1], vec![ListEntity::new(vec![1]).with_pinned(true)])
        .unwrap()
        .allowing_unassigned();
    let a = p.initial_assignment();
    let index = ListDestinationIndex::new(&p, &a, true).unwrap();
    let single = index.for_value(&1).unwrap();
    let mut left = SampleRandom::new(17);
    let mut right = left.clone();
    for _ in 0..10 {
        assert_eq!(single.sample(&mut left), Some(None));
    }
    assert_eq!(s.sample(&mut left), s.sample(&mut right));
    let empty = ListDestinationIndex::new(&p, &a, false)
        .unwrap()
        .for_value(&1)
        .unwrap();
    assert!(empty.is_empty());
    assert_eq!(empty.sample(&mut left), None);
}
#[derive(Debug)]
struct Value {
    id: usize,
    clones: Rc<Cell<usize>>,
}
impl Clone for Value {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Value {
    fn eq(&self, b: &Self) -> bool {
        self.id == b.id
    }
}
impl Eq for Value {}
#[test]
fn index_queries_do_not_clone_values_and_owned_results_are_stable_snapshots() {
    let count = Rc::new(Cell::new(0));
    let v = Value {
        id: 1,
        clones: count.clone(),
    };
    let p = ListProblem::new(
        vec![v.clone()],
        vec![ListEntity::new(vec![v.clone()]), ListEntity::new(vec![])],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    count.set(0);
    let old = {
        let index = ListDestinationIndex::new(&p, &a, false).unwrap();
        index.for_value(&v).unwrap()
    };
    assert_eq!(count.get(), 0);
    assert_eq!(
        old.iter().collect::<Vec<_>>(),
        vec![pos(0, 0), pos(0, 1), pos(1, 0)]
    );
    p.apply(
        &mut a,
        &ListMove::Change {
            source: ListPosition {
                entity: 0,
                index: 0,
            },
            destination: ListPosition {
                entity: 1,
                index: 0,
            },
        },
    )
    .unwrap()
    .commit();
    let new = ListDestinationIndex::new(&p, &a, false)
        .unwrap()
        .for_value(&v)
        .unwrap();
    assert_eq!(
        new.iter().collect::<Vec<_>>(),
        vec![pos(0, 0), pos(1, 0), pos(1, 1)]
    );
    assert_eq!(
        old.iter().collect::<Vec<_>>(),
        vec![pos(0, 0), pos(0, 1), pos(1, 0)]
    );
}
#[test]
fn index_matches_direct_slot_filtering_across_all_small_range_masks() {
    for mask in 0..64usize {
        let ranges: Vec<Vec<_>> = (0..3)
            .map(|e| (0..2).filter(|v| mask & (1 << (e * 2 + v)) != 0).collect())
            .collect();
        let p = ListProblem::new(
            vec![0, 1],
            ranges
                .iter()
                .map(|r| ListEntity::new(vec![]).with_value_range(r.clone()))
                .collect(),
        )
        .unwrap();
        let a = p.initial_assignment();
        let index = ListDestinationIndex::new(&p, &a, false).unwrap();
        for group in [vec![], vec![0], vec![1], vec![0, 1]] {
            let expected: Vec<_> = (0..3)
                .filter(|&e| group.iter().all(|v| ranges[e].contains(v)))
                .map(|e| pos(e, 0))
                .collect();
            assert_eq!(
                index.for_values(&group).unwrap().iter().collect::<Vec<_>>(),
                expected
            );
        }
    }
    let p = ListProblem::new(vec![1], vec![]).unwrap();
    let a = p.initial_assignment();
    assert!(
        ListDestinationIndex::new(&p, &a, false)
            .unwrap()
            .for_value(&1)
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        ListDestinationIndex::new(&p, &a, true),
        Err(ListDestinationError::Model(
            ListError::UnassignmentNotAllowed
        ))
    ));
    assert!(ListDestinationIndex::new(&p, &ListAssignment::new(vec![vec![]]), false).is_err());
}
#[test]
fn indexed_group_destinations_drive_search_on_both_scoring_backends() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1, 2]).with_value_range(vec![1, 2]),
            ListEntity::new(vec![]).with_value_range(vec![1, 2]),
            ListEntity::new(vec![]).with_value_range(vec![2]),
            ListEntity::new(vec![3]).with_pinned(true),
        ],
    )
    .unwrap();
    let mut network = ListConstraintNetwork::new(4, 3);
    network
        .add_constraint("target", vec![ListScope::Entity(1)], |facts| {
            let ListFact::Entity(row) = facts[0] else {
                unreachable!()
            };
            SimpleScore::new(row.len() as i64)
        })
        .unwrap();
    for stream in [false, true] {
        let mut provider = |p: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| {
            ListDestinationIndex::new(p, a, false)
                .unwrap()
                .for_values(&[1, 2])
                .unwrap()
                .iter()
                .filter_map(|destination| p.gather_move(a, &[1, 2], destination).ok())
                .collect::<Vec<_>>()
        };
        let config = LocalSearchConfig {
            max_steps: 2,
            max_evaluations: 100,
            assert_scores: true,
            ..Default::default()
        };
        let r = if stream {
            let mut b = StreamGraphBuilder::new();
            let sources = ListStreamSources::new(&mut b);
            b.score("target", sources.entities.port(), |(e, row)| {
                SimpleScore::new(if *e == 1 { row.len() as i64 } else { 0 })
            })
            .unwrap();
            list_stream_local_search_from_with_provider(
                &p,
                b.build(),
                sources,
                p.initial_assignment(),
                config,
                &mut provider,
            )
        } else {
            list_local_search_from_with_provider(
                &p,
                &network,
                p.initial_assignment(),
                config,
                &mut provider,
            )
        }
        .unwrap();
        let best = r.best.unwrap();
        assert_eq!(best.score, SimpleScore::new(2));
        assert_eq!(
            best.assignment.lists(),
            &[vec![], vec![1, 2], vec![], vec![3]]
        );
    }
}
