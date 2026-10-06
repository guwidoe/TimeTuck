//! Original cached-range, atomicity, reset and native solver integration checks.
use ManagedRangeTarget::{Entity, Solution};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use timetuck::*;
#[test]
fn equal_ranges_share_sort_state_and_snapshots_survive_replacement() {
    let mut m = ManagedValueRanges::entity_provided(
        ManagedRangeKind::Basic {
            allows_unassigned: true,
        },
        vec![
            vec![vec![2, 1, 2], vec![3]],
            vec![vec![2, 1, 3]],
            vec![vec![1, 3]],
        ],
    );
    let initial = m.get(Entity(0)).unwrap();
    assert_eq!(initial.values(), &[None, Some(2), Some(1), Some(3)]);
    assert!(Arc::ptr_eq(&initial, &m.get(Entity(1)).unwrap()));
    assert!(!Arc::ptr_eq(&initial, &m.get(Entity(2)).unwrap()));
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let descending = ManagedRangeOrder::new(move |a: &i32, b: &i32| {
        counter.fetch_add(1, Ordering::Relaxed);
        b.cmp(a)
    });
    let sorted = m.sort(Entity(0), &descending).unwrap();
    assert_eq!(sorted.values(), &[None, Some(3), Some(2), Some(1)]);
    let n = calls.load(Ordering::Relaxed);
    assert!(n > 0);
    assert!(Arc::ptr_eq(
        &sorted,
        &m.sort(Entity(1), &descending.clone()).unwrap()
    ));
    assert_eq!(calls.load(Ordering::Relaxed), n);
    let raw = m.raw(Entity(0)).unwrap();
    assert_eq!(raw.values(), &[2, 1, 3]);
    let panic_order = ManagedRangeOrder::new(|_: &i32, _: &i32| panic!("sort failed"));
    assert!(catch_unwind(AssertUnwindSafe(|| m.sort(Entity(0), &panic_order))).is_err());
    assert!(Arc::ptr_eq(&sorted, &m.get(Entity(0)).unwrap()));
    assert!(Arc::ptr_eq(
        &sorted,
        &m.sort(Entity(0), &descending).unwrap()
    ));
    assert_eq!(initial.values(), &[None, Some(2), Some(1), Some(3)]);
    m.reset_shared(vec![vec![8, 9]], 1);
    assert_eq!(m.get(Solution).unwrap().values(), &[None, Some(8), Some(9)]);
    assert_eq!(sorted.values(), &[None, Some(3), Some(2), Some(1)]);
    assert!(matches!(
        m.get(Entity(1)),
        Err(ManagedRangeError::UnknownEntity(1))
    ));
    m.reset_entity_provided(vec![]);
    assert_eq!(m.entity_count(), 0);
    assert_eq!(m.get(Solution).unwrap().values(), &[None]);
}
#[test]
fn native_basic_solver_uses_sorted_choices_optional_assignments_and_pins() {
    let mut m = ManagedValueRanges::entity_provided(
        ManagedRangeKind::Basic {
            allows_unassigned: false,
        },
        vec![vec![vec![1, 2]], vec![vec![3, 4]]],
    );
    m.sort(
        Entity(0),
        &ManagedRangeOrder::new(|a: &i32, b: &i32| b.cmp(a)),
    )
    .unwrap();
    let p = FiniteProblem::new(vec![
        m.variable(0).unwrap(),
        m.variable(1).unwrap().with_initial(3).with_pinned(true),
    ])
    .unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let r = solve(
        &p,
        &n,
        SolverConfig {
            local_search: vec![],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(r.complete);
    assert_eq!(r.assignment.values(), &[Some(2), Some(3)]);
    m.reset_shared(vec![vec![9]], 2);
    assert_eq!(p.variables()[0].domain(), &[2, 1]);
    let optional = ManagedValueRanges::<i32>::shared(
        ManagedRangeKind::Basic {
            allows_unassigned: true,
        },
        vec![],
        1,
    );
    let p = FiniteProblem::new(vec![optional.variable(0).unwrap()]).unwrap();
    let r = solve(
        &p,
        &ConstraintNetwork::<_, SimpleScore>::new(1),
        SolverConfig {
            local_search: vec![],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(r.complete);
    assert_eq!(r.assignment.values(), &[None]);
    assert!(matches!(
        optional.list_problem(vec![]),
        Err(ManagedRangeError::WrongVariableKind)
    ));
}
#[test]
fn native_list_solver_honors_managed_ranges_and_preserves_pin_metadata() {
    for optional in [false, true] {
        let m = ManagedValueRanges::entity_provided(
            ManagedRangeKind::List {
                allows_unassigned: optional,
            },
            vec![vec![vec![0, 2]], vec![vec![1, 2]], vec![]],
        );
        assert!(
            m.get(Solution)
                .unwrap()
                .values()
                .iter()
                .all(Option::is_some)
        );
        let p = m
            .list_problem(vec![
                ListEntity::new(vec![0]).with_pinned_prefix(1),
                ListEntity::new(vec![1]).with_pinned(true),
                ListEntity::new(vec![]),
            ])
            .unwrap();
        assert_eq!(p.allows_unassigned(), optional);
        assert_eq!(p.entities()[0].pinned_prefix(), 1);
        assert!(p.entities()[1].is_pinned());
        let n = ListConstraintNetwork::<_, SimpleScore>::new(3, 3);
        let r = solve_list(
            &p,
            &n,
            SolverConfig {
                local_search: vec![],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(r.complete);
        p.validate(&r.assignment).unwrap();
        assert_eq!(r.assignment.lists()[0][0], 0);
        assert_eq!(r.assignment.lists()[1], vec![1]);
        assert!(r.assignment.lists()[2].is_empty());
        if !optional {
            assert_eq!(r.assignment.lists()[0], vec![0, 2]);
        }
        assert!(matches!(
            m.variable(0),
            Err(ManagedRangeError::WrongVariableKind)
        ));
        assert!(matches!(
            m.list_problem(vec![]),
            Err(ManagedRangeError::EntityCount { .. })
        ));
        assert!(
            m.list_problem(vec![
                ListEntity::new(vec![1]),
                ListEntity::new(vec![]),
                ListEntity::new(vec![])
            ])
            .is_err()
        );
    }
}
#[test]
fn all_small_range_unions_preserve_membership_and_only_equal_orders_share() {
    for left in 0..16 {
        for right in 0..16 {
            let values = |mask| {
                (0..4)
                    .filter(|i| mask & (1 << i) != 0)
                    .collect::<Vec<i32>>()
            };
            let a = values(left);
            let b = values(right);
            let m = ManagedValueRanges::entity_provided(
                ManagedRangeKind::Basic {
                    allows_unassigned: false,
                },
                vec![vec![a.clone(), a.clone()], vec![b.clone()]],
            );
            let mut union = m.raw(Solution).unwrap().values().to_vec();
            union.sort();
            assert_eq!(union, values(left | right));
            assert_eq!(
                Arc::ptr_eq(&m.get(Entity(0)).unwrap(), &m.get(Entity(1)).unwrap()),
                left == right
            );
            assert_eq!(m.variable(0).unwrap().domain(), a);
            assert_eq!(m.variable(1).unwrap().domain(), b);
        }
    }
    let empty = ManagedValueRanges::shared(
        ManagedRangeKind::List {
            allows_unassigned: true,
        },
        vec![vec![1, 2]],
        0,
    );
    assert_eq!(empty.raw(Solution).unwrap().values(), &[1, 2]);
    assert!(empty.list_problem(vec![]).unwrap().allows_unassigned());
}
