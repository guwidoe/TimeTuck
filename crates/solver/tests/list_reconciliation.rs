//! Original reconciliation regressions; no upstream method credit.
use timetuck::*;
#[test]
fn pins_ranges_and_retained_ownership_take_precedence_independent_of_entity_order() {
    let old = ListAssignmentSnapshot::new(
        vec!["a", "b", "gone"],
        ListAssignment::new(vec![vec![0, 1], vec![2, 3], vec![4]]),
    )
    .unwrap();
    // New entity first claims 2 as a default, but b retains it. Pin 0 on b wins
    // over a's old ownership; removed entity's 4 remains missing, as does new 5.
    let p = ListProblem::new(
        vec![0, 2, 3, 4, 5],
        vec![
            ListEntity::new(vec![2]),
            ListEntity::new(vec![0])
                .with_pinned_prefix(1)
                .with_value_range(vec![0, 2, 5]),
            ListEntity::new(vec![3]).with_value_range(vec![3, 4]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let r = reconcile_list_problem(&p, &["new", "b", "a"], Some(&old)).unwrap();
    assert_eq!(
        r.initial_assignment().lists(),
        &[vec![], vec![0, 2], vec![]]
    );
    assert!(r.allows_unassigned());
    assert_eq!(r.entities()[1].pinned_prefix(), 1);
    assert_eq!(r.entities()[1].value_range(), Some([0, 2, 5].as_slice()));
    assert!(r.is_complete(&r.initial_assignment()));
    assert_eq!(p.initial_assignment().lists(), &[vec![2], vec![0], vec![3]]);
    assert_eq!(old.assignment().lists(), &[vec![0, 1], vec![2, 3], vec![4]]);
    let r = reconcile_list_problem::<&str, _>(&p, &["new", "b", "a"], None).unwrap();
    assert_eq!(r.initial_assignment(), p.initial_assignment());
}
#[test]
fn full_pins_new_entities_and_existing_empty_lists_remain_distinct() {
    let old =
        ListAssignmentSnapshot::new(vec![1, 2], ListAssignment::new(vec![vec![0, 1, 2], vec![]]))
            .unwrap();
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![
            ListEntity::new(vec![2]).with_pinned(true),
            ListEntity::new(vec![3]),
            ListEntity::new(vec![0]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let r = reconcile_list_problem(&p, &[9, 8, 2, 1], Some(&old)).unwrap();
    assert_eq!(
        r.initial_assignment().lists(),
        &[vec![2], vec![3], vec![], vec![0, 1]]
    );
    assert!(r.entities()[0].is_pinned());
    assert!(r.is_complete(&r.initial_assignment()));
    let deleted = ListProblem::new(vec![0, 1, 2], vec![]).unwrap();
    assert!(
        reconcile_list_problem(&deleted, &[], Some(&old))
            .unwrap()
            .entities()
            .is_empty()
    );
}
#[test]
fn dependency_repair_preserves_metadata_and_rejects_pinned_cycles() {
    let old =
        ListAssignmentSnapshot::new(vec![1], ListAssignment::new(vec![vec![1, 0, 2, 3]])).unwrap();
    let p = ListProblem::new(vec![0, 1, 2, 3], vec![ListEntity::new(vec![])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0], vec![1], vec![]])
        .unwrap();
    let r = reconcile_list_problem(&p, &[1], Some(&old)).unwrap();
    // Dynamic predecessor dependencies also make 3 downstream of the cycle.
    assert!(r.initial_assignment().lists()[0].is_empty());
    assert!(matches!(
        r.validate(old.assignment()),
        Err(ListError::DependencyCycle { .. })
    ));
    let pinned = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![ListEntity::new(vec![1]).with_pinned_prefix(1)],
    )
    .unwrap()
    .with_dependencies(vec![vec![], vec![0], vec![1], vec![]])
    .unwrap();
    assert!(matches!(
        reconcile_list_problem(&pinned, &[1], Some(&old)),
        Err(ListReconciliationError::Model(
            ListError::PinnedDependencyCycle { .. }
        ))
    ));
    assert_eq!(pinned.initial_assignment().lists(), &[vec![1]]);
}
#[derive(Clone, Debug)]
struct Value {
    id: i32,
    fact: i32,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
#[test]
fn current_registry_objects_replace_old_facts_in_both_pins_and_retained_values() {
    let old = ListAssignmentSnapshot::new(
        vec![1],
        ListAssignment::new(vec![vec![
            Value { id: 1, fact: 1 },
            Value { id: 2, fact: 2 },
        ]]),
    )
    .unwrap();
    let p = ListProblem::new(
        vec![Value { id: 1, fact: 100 }, Value { id: 2, fact: 200 }],
        vec![ListEntity::new(vec![Value { id: 1, fact: 10 }]).with_pinned_prefix(1)],
    )
    .unwrap();
    let r = reconcile_list_problem(&p, &[1], Some(&old)).unwrap();
    assert_eq!(
        r.initial_assignment().lists()[0]
            .iter()
            .map(|v| v.fact)
            .collect::<Vec<_>>(),
        vec![100, 200]
    );
    assert_eq!(old.assignment().lists()[0][0].fact, 1);
}
#[test]
fn duplicate_ids_values_and_length_mismatches_are_rejected() {
    assert_eq!(
        ListAssignmentSnapshot::new(vec![1], ListAssignment::<i32>::new(vec![])).unwrap_err(),
        ListReconciliationError::IdentityCount {
            expected: 0,
            actual: 1
        }
    );
    assert_eq!(
        ListAssignmentSnapshot::new(vec![1, 1], ListAssignment::<i32>::new(vec![vec![], vec![]]))
            .unwrap_err(),
        ListReconciliationError::DuplicateIdentity { index: 1 }
    );
    for rows in [vec![vec![1, 1], vec![]], vec![vec![1], vec![1]]] {
        assert_eq!(
            ListAssignmentSnapshot::new(vec![1, 2], ListAssignment::new(rows)).unwrap_err(),
            ListReconciliationError::DuplicateValue
        );
    }
    let p = ListProblem::<i32>::new(vec![], vec![ListEntity::new(vec![]); 2]).unwrap();
    assert_eq!(
        reconcile_list_problem::<i32, _>(&p, &[1, 1], None).unwrap_err(),
        ListReconciliationError::DuplicateIdentity { index: 1 }
    );
    assert_eq!(
        reconcile_list_problem::<i32, _>(&p, &[1], None).unwrap_err(),
        ListReconciliationError::IdentityCount {
            expected: 2,
            actual: 1
        }
    );
}
