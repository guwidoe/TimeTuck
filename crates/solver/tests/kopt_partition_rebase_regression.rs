//! Original inferred partition and snapshot/lookup failure controls.
use timetuck::*;
fn source() -> ListProblem<u8> {
    ListProblem::new((0..6).collect(), vec![ListEntity::new((0..6).collect())]).unwrap()
}
fn plan(p: &ListProblem<u8>) -> ListKOptEdgePlan<u8> {
    ListKOptEdgePlan::prepare(
        p,
        &p.initial_assignment(),
        0,
        &[(5, 0), (1, 2), (3, 4)],
        &[(0, 2), (1, 4), (3, 5)],
    )
    .unwrap()
}
#[test]
fn rebasing_checks_all_ordered_values_and_rejects_collisions_stale_rows_and_pins() {
    let p = source();
    let edge_plan = plan(&p);
    let target = ListProblem::new(
        (10..16u16).collect(),
        vec![ListEntity::new(vec![]), ListEntity::new((10..16).collect())],
    )
    .unwrap();
    let a = target.initial_assignment();
    let mut entities = 0;
    let mut values = 0;
    let rebased = edge_plan
        .rebase(
            &target,
            &a,
            |_| {
                entities += 1;
                Ok::<_, ()>(1)
            },
            |v| {
                values += 1;
                Ok(u16::from(*v) + 10)
            },
        )
        .unwrap();
    assert_eq!((entities, values), (1, 6));
    let movement = rebased.movement(&target, &a).unwrap();
    let mut trial = a.clone();
    {
        let applied = target.apply(&mut trial, &movement).unwrap();
        assert_eq!(applied.lists()[1], vec![10, 11, 14, 15, 13, 12]);
    }
    assert_eq!(trial, a);
    assert!(matches!(
        edge_plan.rebase(&target, &a, |_| Ok::<_, ()>(1), |_| Ok(10)),
        Err(ListKOptRebaseError::Collision(
            MoveRebaseNamespace::ListValue
        ))
    ));
    let stale = ListAssignment::new(vec![vec![], vec![11, 10, 12, 13, 14, 15]]);
    assert!(matches!(
        edge_plan.rebase(
            &target,
            &stale,
            |_| Ok::<_, ()>(1),
            |v| Ok(u16::from(*v) + 10)
        ),
        Err(ListKOptRebaseError::Topology(ListKOptError::Partition))
    ));
    let pinned = ListProblem::new(
        (10..16u16).collect(),
        vec![ListEntity::new((10..16).collect()).with_pinned_prefix(1)],
    )
    .unwrap();
    assert!(matches!(
        edge_plan.rebase(
            &pinned,
            &pinned.initial_assignment(),
            |_| Ok::<_, ()>(0),
            |v| Ok(u16::from(*v) + 10)
        ),
        Err(ListKOptRebaseError::Topology(ListKOptError::Partition))
    ));
    assert!(matches!(
        edge_plan.rebase(&target, &a, |_| Ok::<_, ()>(1), |_| Err(())),
        Err(ListKOptRebaseError::Lookup(()))
    ));
    assert_eq!(a, target.initial_assignment());
}
#[test]
fn inferred_sections_keep_pins_membership_and_rotate_the_anchor_crossing_section() {
    let p = source();
    let a = p.initial_assignment();
    let m = list_k_opt_inferred_partition_move(
        &p,
        &a,
        &[0],
        &[(5, 0), (1, 2), (3, 4)],
        &[(0, 2), (1, 4), (3, 5)],
    )
    .unwrap();
    let mut result = a.clone();
    p.apply(&mut result, &m).unwrap().commit();
    assert_eq!(result.lists()[0], vec![0, 1, 4, 5, 3, 2]);
    let p = ListProblem::new(
        (0..9).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3, 4]).with_pinned_prefix(1),
            ListEntity::new(vec![5, 6, 7, 8]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let m =
        list_k_opt_inferred_partition_move(&p, &a, &[0, 1], &[(1, 2), (5, 6)], &[(1, 5), (2, 6)])
            .unwrap();
    let mut result = a.clone();
    p.apply(&mut result, &m).unwrap().commit();
    assert_eq!(result.lists()[0][0], 0);
    assert_eq!(result.lists().iter().map(Vec::len).sum::<usize>(), 9);
    p.validate(&result).unwrap();
}
#[test]
fn invalid_inferred_plans_never_mutate_input_and_edge_plans_reject_stale_execution() {
    let p = source();
    let mut a = p.initial_assignment();
    let initial = a.clone();
    let e = plan(&p);
    p.apply(
        &mut a,
        &ListMove::Swap {
            left: ListPosition {
                entity: 0,
                index: 0,
            },
            right: ListPosition {
                entity: 0,
                index: 1,
            },
        },
    )
    .unwrap()
    .commit();
    assert!(matches!(e.movement(&p, &a), Err(ListKOptError::Partition)));
    assert!(
        list_k_opt_inferred_partition_move(
            &p,
            &initial,
            &[0, 0],
            &[(5, 0), (1, 2)],
            &[(0, 2), (1, 5)]
        )
        .is_err()
    );
    assert_eq!(initial, p.initial_assignment());
}
