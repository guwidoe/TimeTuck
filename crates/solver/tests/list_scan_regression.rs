//! Original snapshot and complexity regressions; no upstream method credit.
use std::{
    cell::Cell,
    hash::{Hash, Hasher},
    rc::Rc,
};
use timetuck::*;

#[test]
fn bulk_anchors_and_receivers_preserve_order_duplicates_pins_and_missing_values() {
    let values: Vec<_> = (0..64).collect();
    let entities = vec![
        ListEntity::new((0..16).collect())
            .with_pinned_prefix(3)
            .with_value_range((0..40).collect()),
        ListEntity::new((16..32).collect()).with_pinned(true),
        ListEntity::new((32..48).collect()).with_value_range((16..64).collect()),
    ];
    let linear = ListProblem::new(values.clone(), entities.clone())
        .unwrap()
        .allowing_unassigned();
    let indexed = ListProblem::new_indexed(values, entities)
        .unwrap()
        .allowing_unassigned();
    let a = linear.initial_assignment();
    for anchors in [vec![1, 47], (0..64).rev().chain([4, 4, 35]).collect()] {
        let x = ListDestinationSelector::new(&linear, &a, &[2, 0, 2, 1], &anchors, true).unwrap();
        let y = ListDestinationSelector::new(&indexed, &a, &[2, 0, 2, 1], &anchors, true).unwrap();
        for group in [vec![], vec![4], vec![35, 35], vec![4, 35], vec![63]] {
            assert_eq!(x.for_values(&group), y.for_values(&group));
            let dx = ListDestinationIndex::new(&linear, &a, true)
                .unwrap()
                .for_values(&group)
                .unwrap();
            let dy = ListDestinationIndex::new(&indexed, &a, true)
                .unwrap()
                .for_values(&group)
                .unwrap();
            assert_eq!(dx.iter().collect::<Vec<_>>(), dy.iter().collect::<Vec<_>>());
        }
    }
}

#[derive(Debug)]
struct Counted {
    id: usize,
    equals: Rc<Cell<usize>>,
    clones: Rc<Cell<usize>>,
}
impl Clone for Counted {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            equals: self.equals.clone(),
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Counted {
    fn eq(&self, other: &Self) -> bool {
        self.equals.set(self.equals.get() + 1);
        self.id == other.id
    }
}
impl Eq for Counted {}
impl Hash for Counted {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.id.hash(h);
    }
}

#[test]
fn optional_swaps_scan_membership_once_and_clone_only_emitted_replacements() {
    let equals = Rc::new(Cell::new(0));
    let clones = Rc::new(Cell::new(0));
    let n = 256;
    let values: Vec<_> = (0..n)
        .map(|id| Counted {
            id,
            equals: equals.clone(),
            clones: clones.clone(),
        })
        .collect();
    let p = ListProblem::new_indexed(
        values.clone(),
        vec![ListEntity::new(values[..n / 2].to_vec())],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let mut swaps = list_swap_moves(&p, &a).unwrap();
    equals.set(0);
    clones.set(0);
    for _ in 0..n / 2 - 1 {
        assert!(matches!(swaps.next(), Some(ListMove::Swap { .. })));
    }
    assert_eq!(
        equals.get(),
        0,
        "assigned swaps must not open the missing-value snapshot"
    );
    assert_eq!(clones.get(), 0);
    assert!(matches!(swaps.next(), Some(ListMove::Composite(_))));
    assert_eq!(clones.get(), 1);
    let remaining = swaps.count();
    assert_eq!(remaining + n / 2, (n / 2) * (n / 2 - 1) + (n / 2) * (n / 2));
    assert_eq!(clones.get(), (n / 2) * (n / 2));
    assert!(
        equals.get() < n * 4,
        "membership work must be linear, got {}",
        equals.get()
    );

    let mut assigns = list_assign_moves(&p, &a).unwrap();
    equals.set(0);
    clones.set(0);
    assert!(matches!(assigns.next(), Some(ListMove::Assign { .. })));
    assert_eq!(clones.get(), 1);
    assert_eq!(assigns.count() + 1, (n / 2) * (n / 2 + 1));
    assert!(equals.get() < n * 4);
}

#[test]
fn indexed_and_eq_only_move_sequences_agree_on_all_small_layouts() {
    for code in 0..81 {
        let mut code = code;
        let mut rows = [vec![], vec![]];
        for value in [3, 0, 2, 1] {
            let owner = code % 3;
            code /= 3;
            if owner < 2 {
                rows[owner].push(value);
            }
        }
        for pinned in [false, true] {
            let entities = vec![
                ListEntity::new(rows[0].clone()).with_pinned_prefix(rows[0].len().min(1)),
                ListEntity::new(rows[1].clone()).with_pinned(pinned),
            ];
            let linear = ListProblem::new(vec![3, 0, 2, 1], entities.clone())
                .unwrap()
                .allowing_unassigned();
            let indexed = ListProblem::new_indexed(vec![3, 0, 2, 1], entities)
                .unwrap()
                .allowing_unassigned();
            let a = linear.initial_assignment();
            for family in [
                Neighborhood::Swap,
                Neighborhood::UniqueSwap,
                Neighborhood::Change,
            ] {
                assert_eq!(
                    list_moves(&linear, &a, family).unwrap().collect::<Vec<_>>(),
                    list_moves(&indexed, &a, family)
                        .unwrap()
                        .collect::<Vec<_>>()
                );
            }
            assert_eq!(
                list_assign_moves(&linear, &a).unwrap().collect::<Vec<_>>(),
                list_assign_moves(&indexed, &a).unwrap().collect::<Vec<_>>()
            );
        }
    }
    fn send_sync(_: impl Send + Sync) {}
    let p = ListProblem::new_indexed(vec![0, 1], vec![ListEntity::new(vec![0])])
        .unwrap()
        .allowing_unassigned();
    let a = p.initial_assignment();
    send_sync(list_swap_moves(&p, &a).unwrap());
    send_sync(list_assign_moves(&p, &a).unwrap());
}

#[test]
fn construction_gather_and_repair_keep_identical_results_and_work_counts() {
    let entities = vec![
        ListEntity::new(vec![0, 1])
            .with_pinned_prefix(1)
            .with_value_range(vec![0, 1, 2, 3, 4]),
        ListEntity::new(vec![5, 6]).with_value_range(vec![2, 3, 4, 5, 6, 7]),
    ];
    let linear = ListProblem::new((0..8).collect(), entities.clone()).unwrap();
    let indexed = ListProblem::new_indexed((0..8).collect(), entities).unwrap();
    let mut net = ListConstraintNetwork::new(2, 8);
    for entity in 0..2 {
        net.add_constraint(
            format!("row{entity}"),
            vec![ListScope::Entity(entity)],
            |facts| {
                let ListFact::Entity(row) = facts[0] else {
                    unreachable!()
                };
                SimpleScore::new(
                    row.iter()
                        .enumerate()
                        .map(|(i, v)| (i as i64 + 1) * i64::from(*v))
                        .sum(),
                )
            },
        )
        .unwrap();
    }
    for optional in [false, true] {
        let (p, q) = if optional {
            (
                linear.clone().allowing_unassigned(),
                indexed.clone().allowing_unassigned(),
            )
        } else {
            (linear.clone(), indexed.clone())
        };
        for placement in [
            ConstructionPlacement::Sequential,
            ConstructionPlacement::Pooled,
        ] {
            for policy in [
                OptionalConstruction::Preserve,
                OptionalConstruction::NonWorsening,
                OptionalConstruction::EvaluateUnassigned,
            ] {
                for limit in [Some(1), Some(7), None] {
                    let c = ConstructionConfig {
                        placement,
                        optional: policy,
                        max_move_evaluations: limit,
                        assert_scores: true,
                        ..Default::default()
                    };
                    assert_eq!(
                        greedy_list_construction(&p, &net, c).unwrap(),
                        greedy_list_construction(&q, &net, c).unwrap()
                    );
                }
            }
        }
        for seed in [0, 1, 917] {
            let mut c = GatherSamplingConfig::new(1, 4, 50, seed).unwrap();
            c.unassignment = optional;
            let a = p.initial_assignment();
            assert_eq!(
                sampled_list_gather_moves(&p, &a, c)
                    .unwrap()
                    .collect::<Vec<_>>(),
                sampled_list_gather_moves(&q, &a, c)
                    .unwrap()
                    .collect::<Vec<_>>()
            );
            let a = greedy_list_construction(&p, &net, ConstructionConfig::default())
                .unwrap()
                .assignment;
            let c = RuinRecreateConfig {
                seed,
                minimum_ruined: 1,
                maximum_ruined: 4,
                max_attempts: 3,
                assert_scores: true,
                ..Default::default()
            };
            assert_eq!(
                ruin_recreate_list_search_from(&p, &net, a.clone(), c.clone()).unwrap(),
                ruin_recreate_list_search_from(&q, &net, a, c).unwrap()
            );
        }
    }
}
