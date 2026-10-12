// Adapted pinned SelectorBasedKOptListMoveTest and TEST-only list fixtures.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::sync::Arc;
use timetuck::*;
#[derive(Debug, Eq, PartialEq)]
struct Value {
    code: usize,
}
struct Owner {
    code: usize,
    values: Vec<Arc<Value>>,
}
fn values(n: usize) -> Vec<Arc<Value>> {
    (1..=n).map(|code| Arc::new(Value { code })).collect()
}
fn problem(values: &[Arc<Value>], rows: &[Vec<usize>]) -> ListProblem<Arc<Value>> {
    ListProblem::new(
        values.to_vec(),
        rows.iter()
            .map(|r| ListEntity::new(r.iter().map(|&v| values[v - 1].clone()).collect()))
            .collect(),
    )
    .unwrap()
}
fn edges(pairs: &[(usize, usize)]) -> Vec<(usize, usize)> {
    pairs.iter().map(|&(a, b)| (a - 1, b - 1)).collect()
}
fn observe(p: &ListProblem<Arc<Value>>, movement: ListMove<Arc<Value>>, expected: &[Vec<usize>]) {
    let mut a = p.initial_assignment();
    let original = a.clone();
    let old_lengths: Vec<_> = a.lists().iter().map(Vec::len).collect();
    let plan = ListLifecyclePlan::prepare_with_scope(
        p,
        &a,
        movement,
        ListNotificationScope::MovableSuffix,
    )
    .unwrap();
    let mut notifications = vec![];
    plan.apply(
        &mut a,
        &mut |e: ListLifecycleEvent<'_, Arc<Value>>| match e.change {
            ListLifecycleChange::Changed(range) => {
                assert!(!e.undo);
                assert_eq!(range.start, 0);
                notifications.push((e.phase, range));
            }
            ListLifecycleChange::Flush => {}
            _ => panic!("reconnection retains membership"),
        },
    )
    .unwrap()
    .commit();
    assert_eq!(
        a.lists()
            .iter()
            .map(|r| r.iter().map(|v| v.code).collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        expected
    );
    for (entity, (old, new)) in old_lengths.iter().zip(expected).enumerate() {
        assert!(notifications.contains(&(
            ListChangePhase::Before,
            ListRange {
                entity,
                start: 0,
                end: *old
            }
        )));
        assert!(notifications.contains(&(
            ListChangePhase::After,
            ListRange {
                entity,
                start: 0,
                end: new.len()
            }
        )));
    }
    assert_eq!(notifications.len(), 2 * expected.len());
    for row in a.lists() {
        for v in row {
            assert!(Arc::ptr_eq(
                v,
                p.values().iter().find(|x| x.code == v.code).unwrap()
            ));
        }
    }
    assert_eq!(original, p.initial_assignment());
}
#[test]
fn test_multi_entity2_opt() {
    let v = values(8);
    let p = problem(&v, &[vec![1, 2, 3, 4], vec![5, 6, 7, 8]]);
    let movement = list_k_opt_inferred_partition_move(
        &p,
        &p.initial_assignment(),
        &[0, 1],
        &edges(&[(2, 3), (6, 7)]),
        &edges(&[(2, 6), (3, 7)]),
    )
    .unwrap();
    observe(&p, movement, &[vec![1, 2, 6, 5, 4], vec![3, 7, 8]]);
}
#[test]
fn test_multi_entity4_opt() {
    let v = values(12);
    let p = problem(
        &v,
        &[vec![1, 2, 3, 4], vec![5, 6, 7, 8], vec![9, 10, 11, 12]],
    );
    let movement = list_k_opt_inferred_partition_move(
        &p,
        &p.initial_assignment(),
        &[0, 1, 2],
        &edges(&[(2, 3), (7, 8), (9, 10), (11, 12)]),
        &edges(&[(11, 3), (12, 7), (2, 9), (10, 8)]),
    )
    .unwrap();
    observe(
        &p,
        movement,
        &[vec![1, 2, 9, 8], vec![10, 11, 3, 4], vec![5, 6, 7, 12]],
    );
}
#[test]
fn test3_opt_rebase() {
    let source_values = values(6);
    let destination_values = values(6);
    let source_owner = Arc::new(Owner {
        code: 1,
        values: source_values.clone(),
    });
    let destination_owner = Arc::new(Owner {
        code: 1,
        values: destination_values.clone(),
    });
    let source = ListProblem::new(
        source_values.clone(),
        vec![ListEntity::new(source_owner.values.clone())],
    )
    .unwrap();
    let destination = ListProblem::new(
        destination_values.clone(),
        vec![ListEntity::new(destination_owner.values.clone())],
    )
    .unwrap();
    let mut lookup = WorkingObjectLookup::new(
        WorkingLookupStrategy::IdOrFail,
        WorkingLookupSchema::id("code", |v: &Value| Some(v.code)),
    );
    lookup
        .reset_with(destination_values.iter().cloned())
        .unwrap();
    let mut owners = WorkingObjectLookup::new(
        WorkingLookupStrategy::IdOrFail,
        WorkingLookupSchema::id("code", |v: &Owner| Some(v.code)),
    );
    owners.add(destination_owner.clone()).unwrap();
    let plan = ListKOptEdgePlan::prepare(
        &source,
        &source.initial_assignment(),
        0,
        &edges(&[(6, 1), (2, 3), (4, 5)]),
        &edges(&[(1, 3), (2, 5), (4, 6)]),
    )
    .unwrap();
    let rebased = plan
        .rebase(
            &destination,
            &destination.initial_assignment(),
            |entity| {
                assert_eq!(entity, 0);
                let actual = owners.lookup(&source_owner)?;
                assert!(Arc::ptr_eq(&actual, &destination_owner));
                Ok(std::slice::from_ref(&destination_owner)
                    .iter()
                    .position(|owner| Arc::ptr_eq(owner, &actual))
                    .unwrap())
            },
            |value| lookup.lookup(value),
        )
        .unwrap();
    let movement = rebased
        .movement(&destination, &destination.initial_assignment())
        .unwrap();
    observe(&destination, movement, &[vec![1, 2, 5, 6, 4, 3]]);
    for (s, d) in source_values.iter().zip(&destination_values) {
        assert!(!Arc::ptr_eq(s, d));
    }
    assert_eq!(
        source.initial_assignment().lists()[0]
            .iter()
            .map(|v| v.code)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5, 6]
    );
}
