//! Original range invariants and independent enumeration; no upstream credit.
use std::{cell::Cell, collections::BTreeSet};
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn assign(value: i32, entity: usize, index: usize) -> ListMove<i32> {
    ListMove::Assign {
        value,
        destination: pos(entity, index),
    }
}
fn change(a: usize, i: usize, b: usize, j: usize) -> ListMove<i32> {
    ListMove::Change {
        source: pos(a, i),
        destination: pos(b, j),
    }
}
#[test]
fn metadata_rejects_unknown_duplicate_and_out_of_range_initial_values() {
    for (range, initial, error) in [
        (
            vec![1, 1],
            vec![],
            ListError::DuplicateEntityRangeValue { entity: 0 },
        ),
        (
            vec![3],
            vec![],
            ListError::EntityRangeValueOutsideDomain { entity: 0 },
        ),
        (
            vec![1],
            vec![2],
            ListError::ValueOutsideEntityRange { entity: 0 },
        ),
    ] {
        assert_eq!(
            ListProblem::new(
                vec![1, 2],
                vec![ListEntity::new(initial).with_value_range(range)]
            )
            .unwrap_err(),
            error
        );
    }
    let p = ListProblem::new(
        vec![1, 2],
        vec![
            ListEntity::new(vec![1]),
            ListEntity::new(vec![]).with_value_range(vec![]),
        ],
    )
    .unwrap();
    assert_eq!(p.entities()[0].value_range(), None);
    assert_eq!(p.entities()[1].value_range(), Some([].as_slice()));
    assert!(
        p.validate(&ListAssignment::new(vec![vec![1, 2], vec![]]))
            .is_ok()
    );
    assert_eq!(
        p.validate(&ListAssignment::new(vec![vec![1], vec![2]])),
        Err(ListError::ValueOutsideEntityRange { entity: 1 })
    );
}
#[test]
fn disallowed_assign_relocation_and_both_swap_directions_roll_back() {
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![
            ListEntity::new(vec![0]).with_value_range(vec![0, 2]),
            ListEntity::new(vec![1]).with_value_range(vec![1]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let mut working = a.clone();
    for movement in [
        assign(2, 1, 0),
        change(0, 0, 1, 0),
        change(1, 0, 0, 0),
        ListMove::Swap {
            left: pos(0, 0),
            right: pos(1, 0),
        },
        ListMove::Swap {
            left: pos(1, 0),
            right: pos(0, 0),
        },
        ListMove::Composite(vec![assign(2, 0, 1), change(0, 0, 1, 0)]),
    ] {
        assert!(matches!(
            p.apply(&mut working, &movement),
            Err(ListError::ValueOutsideEntityRange { .. })
        ));
        assert_eq!(working, a);
    }
}
#[test]
fn composites_validate_ranges_on_final_state_and_scoring_rejects_before_callbacks() {
    let p = ListProblem::new(
        vec![0, 1],
        vec![
            ListEntity::new(vec![0]).with_value_range(vec![0]),
            ListEntity::new(vec![1]).with_value_range(vec![1]),
        ],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let roundtrip = ListMove::Composite(vec![change(0, 0, 1, 0), change(1, 0, 0, 0)]);
    {
        let trial = p.apply(&mut a, &roundtrip).unwrap();
        assert_eq!(&*trial, &p.initial_assignment());
    }
    let calls = Cell::new(0);
    let mut n = ListConstraintNetwork::new(2, 2);
    n.add_constraint(
        "both",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        |_| {
            calls.set(calls.get() + 1);
            SimpleScore::new(7)
        },
    )
    .unwrap();
    let mut d = ListScoreDirector::new(&p, &n, a).unwrap();
    assert_eq!(calls.get(), 1);
    assert!(d.apply(&change(0, 0, 1, 0)).is_err());
    assert_eq!(calls.get(), 1);
    assert_eq!(d.assignment(), &p.initial_assignment());
    assert_eq!(d.score(), SimpleScore::new(7));
    d.assert_consistent().unwrap();
}
fn permutations() -> [[i32; 3]; 6] {
    [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ]
}
fn states(ranges: &[Vec<i32>; 2], optional: bool) -> BTreeSet<Vec<Vec<i32>>> {
    let mut all = BTreeSet::new();
    for owners in 0..27 {
        let owner = [owners % 3, (owners / 3) % 3, owners / 9];
        if (!optional && owner.contains(&2))
            || (0..3).any(|v| owner[v] < 2 && !ranges[owner[v]].contains(&(v as i32)))
        {
            continue;
        }
        for perm in permutations() {
            let mut rows = vec![vec![], vec![]];
            for v in perm {
                if owner[v as usize] < 2 {
                    rows[owner[v as usize]].push(v);
                }
            }
            all.insert(rows);
        }
    }
    all
}
#[test]
fn exact_enumeration_matches_all_owner_permutations_for_every_three_value_range_pair() {
    for bits in 0..64 {
        let ranges: [Vec<i32>; 2] = std::array::from_fn(|e| {
            (0..3)
                .filter(|&v| bits & (1 << (e * 3 + v)) != 0)
                .map(|v| v as i32)
                .collect()
        });
        for optional in [false, true] {
            let p = ListProblem::new(
                vec![0, 1, 2],
                ranges
                    .iter()
                    .map(|r| ListEntity::new(vec![]).with_value_range(r.clone()))
                    .collect(),
            )
            .unwrap();
            let p = if optional { p.allowing_unassigned() } else { p };
            let expected = states(&ranges, optional);
            let mut actual = BTreeSet::new();
            let result = exhaustive_list_search(
                &p,
                |a| {
                    assert!(p.is_complete(a));
                    assert!(actual.insert(a.lists().to_vec()));
                    SimpleScore::ZERO
                },
                1000,
            );
            assert_eq!(actual, expected, "ranges {ranges:?}, optional {optional}");
            assert_eq!(result.evaluations, actual.len() as u64);
            assert_eq!(result.termination, Termination::Exhausted);
            assert_eq!(result.is_optimal(), !expected.is_empty());
        }
    }
}
#[test]
fn neighborhoods_equal_independent_unrestricted_edits_filtered_by_entity_ranges() {
    // Full unrestricted neighborhoods already have independent vector-edit tests.
    // Here exhaustively compare range filtering, including both swap directions.
    for bits in 0..64 {
        let ranges: [Vec<i32>; 2] = std::array::from_fn(|e| {
            (0..3)
                .filter(|&v| bits & (1 << (e * 3 + v)) != 0)
                .map(|v| v as i32)
                .collect()
        });
        let p = ListProblem::new(
            vec![0, 1, 2],
            ranges
                .iter()
                .map(|r| ListEntity::new(vec![]).with_value_range(r.clone()))
                .collect(),
        )
        .unwrap()
        .allowing_unassigned();
        let plain = ListProblem::new(
            vec![0, 1, 2],
            vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
        )
        .unwrap()
        .allowing_unassigned();
        for lists in states(&ranges, true) {
            let a = ListAssignment::new(lists);
            let mut expected = BTreeSet::new();
            let mut actual = BTreeSet::new();
            for m in list_moves(&plain, &a, Neighborhood::ChangeAndSwap).unwrap() {
                let mut b = a.clone();
                let t = plain.apply(&mut b, &m).unwrap();
                if t.lists()
                    .iter()
                    .enumerate()
                    .all(|(e, row)| row.iter().all(|v| ranges[e].contains(v)))
                {
                    expected.insert(t.lists().to_vec());
                }
            }
            for m in list_moves(&p, &a, Neighborhood::ChangeAndSwap).unwrap() {
                let mut b = a.clone();
                {
                    let t = p.apply(&mut b, &m).unwrap();
                    assert!(p.is_complete(&t));
                    actual.insert(t.lists().to_vec());
                }
                assert_eq!(b, a);
            }
            assert_eq!(actual, expected, "ranges {ranges:?} initial {a:?}");
        }
    }
}
#[test]
fn uncovered_required_values_and_full_pins_prove_impossibility_without_scoring() {
    for p in [
        ListProblem::new(
            vec![0, 1],
            vec![ListEntity::new(vec![]).with_value_range(vec![0])],
        )
        .unwrap(),
        ListProblem::new(
            vec![0, 1],
            vec![
                ListEntity::new(vec![0])
                    .with_value_range(vec![0, 1])
                    .with_pinned(true),
                ListEntity::new(vec![]).with_value_range(vec![]),
            ],
        )
        .unwrap(),
    ] {
        for budget in [0, 100] {
            let r = exhaustive_list_search(
                &p,
                |_| -> SimpleScore { panic!("no complete state") },
                budget,
            );
            assert_eq!(r.termination, Termination::Exhausted);
            assert_eq!(r.evaluations, 0);
            assert!(!r.is_optimal());
            let mut n = ListConstraintNetwork::new(p.entities().len(), 2);
            n.add_constraint("must not score", vec![], |_| -> SimpleScore {
                panic!("impossible")
            })
            .unwrap();
            let c = greedy_list_construction(
                &p,
                &n,
                ConstructionConfig {
                    max_evaluations: budget,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(c.termination, ConstructionTermination::NoCompleteAssignment);
            assert_eq!(c.evaluations, 0);
            assert!(!c.complete);
        }
        let optional = p.allowing_unassigned();
        let r = exhaustive_list_search(&optional, |_| SimpleScore::ZERO, 100);
        assert!(r.is_optimal());
    }
}
#[test]
fn construction_and_search_preserve_ranges_with_non_dense_values_and_prefixes() {
    let p = ListProblem::new(
        vec![10, 20, 30, 40],
        vec![
            ListEntity::new(vec![10])
                .with_pinned_prefix(1)
                .with_value_range(vec![30, 10]),
            ListEntity::new(vec![20]).with_value_range(vec![40, 20]),
            ListEntity::new(vec![]).with_value_range(vec![]),
        ],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(3, 4);
    for e in 0..3 {
        n.add_constraint(format!("cost {e}"), vec![ListScope::Entity(e)], |f| {
            let ListFact::Entity(r) = &f[0] else {
                unreachable!()
            };
            SimpleScore::new(
                r.iter()
                    .enumerate()
                    .map(|(i, &v)| i as i64 * i64::from(v))
                    .sum(),
            )
        })
        .unwrap();
    }
    let c = greedy_list_construction(
        &p,
        &n,
        ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(c.complete);
    assert_eq!(c.evaluations, 4);
    assert_eq!(c.assignment.lists(), &[vec![10, 30], vec![20, 40], vec![]]);
    let r = list_local_search_from(
        &p,
        &n,
        c.assignment,
        LocalSearchConfig {
            assert_scores: true,
            ..Default::default()
        },
    )
    .unwrap();
    let best = r.best.unwrap();
    assert!(p.is_complete(&best.assignment));
    assert_eq!(best.score, SimpleScore::new(70));
}
#[test]
fn exact_budget_and_optional_orphans_are_truthful() {
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![ListEntity::new(vec![]).with_value_range(vec![0, 1])],
    )
    .unwrap()
    .allowing_unassigned();
    let exact = exhaustive_list_search(&p, |_| SimpleScore::ZERO, 5);
    assert_eq!(exact.evaluations, 5);
    assert_eq!(exact.termination, Termination::Exhausted);
    assert!(exact.is_optimal());
    let limited = exhaustive_list_search(&p, |_| SimpleScore::ZERO, 4);
    assert_eq!(limited.evaluations, 4);
    assert_eq!(limited.termination, Termination::EvaluationLimit);
    let a = p.initial_assignment();
    assert!(
        !list_assign_moves(&p, &a)
            .unwrap()
            .any(|m| matches!(m, ListMove::Assign { value: 2, .. }))
    );
}
