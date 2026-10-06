//! Original entity-range registry construction regressions.
use timetuck::*;
#[test]
fn union_preserves_first_identity_order_ranges_pins_and_resume_metadata() {
    let p = ListProblem::from_entity_ranges(vec![
        ListEntity::new(vec![2])
            .with_pinned_prefix(1)
            .with_value_range(vec![2, 1]),
        ListEntity::new(vec![3])
            .with_pinned(true)
            .with_value_range(vec![1, 3]),
        ListEntity::new(vec![]).with_value_range(vec![]),
    ])
    .unwrap()
    .allowing_unassigned();
    assert_eq!(p.values(), &[2, 1, 3]);
    assert_eq!(p.entities()[1].value_range(), Some([1, 3].as_slice()));
    let next = ListAssignment::new(vec![vec![2, 1], vec![3], vec![]]);
    let resumed = p.with_initial_assignment(&next).unwrap();
    assert_eq!(resumed.initial_assignment(), next);
    assert_eq!(resumed.values(), p.values());
    assert!(resumed.allows_unassigned());
    assert_eq!(resumed.entities()[0].pinned_prefix(), 1);
    assert!(resumed.entities()[1].is_pinned());
    assert!(
        p.validate(&ListAssignment::new(vec![vec![2], vec![3], vec![1]]))
            .is_err()
    );
    assert_eq!(p.initial_assignment().lists(), &[vec![2], vec![3], vec![]]);
}
#[test]
fn incomplete_range_metadata_and_invalid_assignments_remain_errors() {
    assert!(matches!(
        ListProblem::from_entity_ranges(vec![ListEntity::<i32>::new(vec![])]),
        Err(ListError::MissingEntityValueRange { entity: 0 })
    ));
    for (entities, error) in [
        (
            vec![ListEntity::new(vec![]).with_value_range(vec![1, 1])],
            ListError::DuplicateEntityRangeValue { entity: 0 },
        ),
        (
            vec![ListEntity::new(vec![2]).with_value_range(vec![1])],
            ListError::ValueOutsideDomain,
        ),
        (
            vec![
                ListEntity::new(vec![2]).with_value_range(vec![1]),
                ListEntity::new(vec![]).with_value_range(vec![2]),
            ],
            ListError::ValueOutsideEntityRange { entity: 0 },
        ),
        (
            vec![
                ListEntity::new(vec![1]).with_value_range(vec![1]),
                ListEntity::new(vec![1]).with_value_range(vec![1]),
            ],
            ListError::DuplicateAssignedValue,
        ),
        (
            vec![
                ListEntity::new(vec![])
                    .with_value_range(vec![1])
                    .with_pinned_prefix(1),
            ],
            ListError::InvalidPin { entity: 0 },
        ),
    ] {
        assert_eq!(
            ListProblem::from_entity_ranges(entities).unwrap_err(),
            error
        );
    }
    let empty = ListProblem::<i32>::from_entity_ranges(vec![]).unwrap();
    assert!(empty.values().is_empty());
    assert!(empty.is_complete(&empty.initial_assignment()));
}
#[test]
fn overlapping_ranges_match_an_independent_union_for_every_small_range_pair() {
    for a in 0..16 {
        for b in 0..16 {
            let range = |mask| {
                (0..4)
                    .filter(|v| mask & (1 << v) != 0)
                    .collect::<Vec<i32>>()
            };
            let p = ListProblem::from_entity_ranges(vec![
                ListEntity::new(vec![]).with_value_range(range(a)),
                ListEntity::new(vec![]).with_value_range(range(b)),
            ])
            .unwrap();
            let mut actual = p.values().to_vec();
            actual.sort();
            assert_eq!(actual, range(a | b));
            let n = ListConstraintNetwork::<_, SimpleScore>::new(2, p.values().len());
            let result = solve_list(
                &p,
                &n,
                SolverConfig {
                    local_search: vec![],
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(result.complete);
            p.validate(&result.assignment).unwrap();
            let mut assigned = result
                .assignment
                .lists()
                .iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            assigned.sort();
            assert_eq!(assigned, range(a | b));
        }
    }
}
