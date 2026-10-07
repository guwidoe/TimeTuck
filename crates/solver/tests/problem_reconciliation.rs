use timetuck::*;
#[test]
fn identities_survive_reordering_insertion_removal_domain_changes_and_pins() {
    let old = AssignmentSnapshot::new(
        vec![10, 20, 30, 40, 50],
        Assignment::new(vec![Some(1), Some(2), None, Some(4), Some(5)]),
    )
    .unwrap();
    let next = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]),              //20 retained despite reorder
        Variable::new(vec![7]).with_initial(7), //new60
        Variable::new(vec![9]).with_initial(9), //10 loses1
        Variable::new(vec![3]).with_initial(3).allowing_unassigned(), //30 retainsNone
        Variable::new(vec![4, 6]).with_initial(6).with_pinned(true), //40 newly pinned
        Variable::new(vec![5]).with_pinned(true), //50 pinned null
    ])
    .unwrap();
    let next = reconcile_problem(&next, &[20, 60, 10, 30, 40, 50], Some(&old)).unwrap();
    assert_eq!(
        next.initial_assignment().values(),
        &[Some(2), Some(7), Some(9), None, Some(6), None]
    );
    assert!(next.variables()[3].allows_unassigned());
    assert!(next.variables()[5].is_pinned());
    let reduced = FiniteProblem::new(vec![Variable::new(vec![2])]).unwrap();
    assert_eq!(
        reconcile_problem(&reduced, &[20], Some(&old))
            .unwrap()
            .initial_assignment()
            .values(),
        &[Some(2)]
    );
    assert_eq!(
        old.assignment().values(),
        &[Some(1), Some(2), None, Some(4), Some(5)]
    );
    assert_eq!(
        reconcile_problem::<i32, _>(&reduced, &[20], None)
            .unwrap()
            .initial_assignment()
            .values(),
        &[None]
    );
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
fn retained_identity_uses_updated_domain_object_and_removed_values_can_be_unassigned() {
    let old = AssignmentSnapshot::new(
        vec!["a", "b"],
        Assignment::new(vec![
            Some(Value { id: 1, fact: 10 }),
            Some(Value { id: 2, fact: 20 }),
        ]),
    )
    .unwrap();
    let model = FiniteProblem::new(vec![
        Variable::new(vec![Value { id: 1, fact: 99 }]),
        Variable::new(vec![]),
    ])
    .unwrap();
    let resumed = reconcile_problem(&model, &["a", "b"], Some(&old)).unwrap();
    let a = resumed.initial_assignment();
    assert_eq!(a.values()[0].as_ref().unwrap().fact, 99);
    assert!(a.values()[1].is_none());
    assert_eq!(old.assignment().values()[0].as_ref().unwrap().fact, 10);
}
#[test]
fn ambiguous_or_missing_identities_are_rejected_before_building_a_model() {
    assert_eq!(
        AssignmentSnapshot::new(vec![1], Assignment::<i32>::new(vec![])).unwrap_err(),
        ReconciliationError::IdentityCount {
            expected: 0,
            actual: 1
        }
    );
    assert_eq!(
        AssignmentSnapshot::new(vec![1, 1], Assignment::<i32>::new(vec![None, None])).unwrap_err(),
        ReconciliationError::DuplicateIdentity { index: 1 }
    );
    let p = FiniteProblem::new(vec![Variable::<i32>::new(vec![]); 2]).unwrap();
    assert_eq!(
        reconcile_problem::<i32, _>(&p, &[1, 1], None).unwrap_err(),
        ReconciliationError::DuplicateIdentity { index: 1 }
    );
    assert_eq!(
        reconcile_problem::<i32, _>(&p, &[1], None).unwrap_err(),
        ReconciliationError::IdentityCount {
            expected: 2,
            actual: 1
        }
    );
    let empty = FiniteProblem::<i32>::new(vec![]).unwrap();
    assert!(
        reconcile_problem::<i32, _>(&empty, &[], None)
            .unwrap()
            .variables()
            .is_empty()
    );
}
