// Adapted from pinned SelectorBasedTwoOptListMoveTest; Apache-2.0.
// See compat/provenance.json for fixtures and native lifecycle substitutions.
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn model(rows: Vec<Vec<i32>>, prefix: usize) -> ListProblem<i32> {
    ListProblem::new(
        rows.iter().flatten().copied().collect(),
        rows.into_iter()
            .enumerate()
            .map(|(i, row)| {
                ListEntity::new(row).with_pinned_prefix(if i == 0 { prefix } else { 0 })
            })
            .collect(),
    )
    .unwrap()
}
fn check(
    before: Vec<Vec<i32>>,
    prefix: usize,
    cuts: (ListPosition, ListPosition),
    expected: Vec<Vec<i32>>,
    ranges: &[(usize, usize, usize, usize)],
) {
    let p = model(before.clone(), prefix);
    let mut a = p.initial_assignment();
    let plan = list_directed_two_opt_plan(&p, &a, cuts.0, cuts.1).unwrap();
    let mut events = vec![];
    plan.apply(
        &mut a,
        &mut |e: ListLifecycleEvent<'_, i32>| match e.change {
            ListLifecycleChange::Changed(r) => {
                assert!(!e.undo);
                assert_eq!(
                    e.assignment.lists(),
                    if e.phase == ListChangePhase::Before {
                        &before
                    } else {
                        &expected
                    }
                );
                events.push((e.phase, r));
            }
            ListLifecycleChange::Flush => {}
            _ => panic!("membership must remain unchanged"),
        },
    )
    .unwrap()
    .commit();
    assert_eq!(a.lists(), expected);
    for &(entity, start, old_end, new_end) in ranges {
        assert!(events.contains(&(
            ListChangePhase::Before,
            ListRange {
                entity,
                start,
                end: old_end
            }
        )));
        assert!(events.contains(&(
            ListChangePhase::After,
            ListRange {
                entity,
                start,
                end: new_end
            }
        )));
    }
    assert_eq!(events.len(), ranges.len() * 2);
}
#[test]
fn do_move() {
    check(
        vec![vec![1, 2, 5, 4, 3, 6, 7, 8]],
        0,
        (pos(0, 2), pos(0, 5)),
        vec![(1..=8).collect()],
        &[(0, 2, 5, 5)],
    );
}
#[test]
fn is_move_doable() {
    let p = model(vec![vec![1, 2, 5, 4, 3, 6, 7, 8]], 0);
    let a = p.initial_assignment();
    for (second, doable) in [(5, true), (2, false), (3, false), (4, true), (1, true)] {
        let plan = list_directed_two_opt_plan(&p, &a, pos(0, 2), pos(0, second)).unwrap();
        assert_eq!(plan.assignment() != &a, doable);
    }
}
#[test]
fn is_move_doable_value_range_provider_on_entity() {
    let p = ListProblem::new(
        (1..=6).collect(),
        vec![
            ListEntity::new(vec![2, 1, 5]).with_value_range(vec![1, 2, 3, 5, 6]),
            ListEntity::new(vec![4, 3, 6]).with_value_range(vec![1, 3, 4, 5, 6]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    assert!(list_directed_two_opt_plan(&p, &a, pos(0, 1), pos(1, 1)).is_ok());
    assert!(list_directed_two_opt_plan(&p, &a, pos(0, 0), pos(1, 1)).is_err());
    assert!(list_directed_two_opt_plan(&p, &a, pos(0, 1), pos(1, 0)).is_err());
}
#[test]
fn is_move_doable_tail_swap() {
    let p = model(vec![vec![1, 2, 3, 4], vec![5, 6, 7, 8, 9]], 0);
    let a = p.initial_assignment();
    for (x, y) in [(2, 2), (1, 2), (2, 1)] {
        assert_ne!(
            list_directed_two_opt_plan(&p, &a, pos(0, x), pos(1, y))
                .unwrap()
                .assignment(),
            &a
        );
    }
}
#[test]
fn do_tail_swap() {
    check(
        vec![vec![1, 2, 3, 4], vec![5, 6, 7, 8, 9]],
        0,
        (pos(0, 2), pos(1, 2)),
        vec![vec![1, 2, 7, 8, 9], vec![5, 6, 3, 4]],
        &[(0, 2, 4, 5), (1, 2, 5, 4)],
    );
}
#[test]
fn do_move_second_ends_before_first() {
    check(
        vec![vec![8, 7, 3, 4, 5, 6, 2, 1]],
        0,
        (pos(0, 6), pos(0, 2)),
        vec![vec![8, 1, 2, 3, 4, 5, 6, 7]],
        &[(0, 0, 8, 8)],
    );
}
#[test]
fn do_move_second_ends_before_first_unbalanced() {
    check(
        vec![vec![5, 2, 3, 4, 1, 7, 6]],
        0,
        (pos(0, 4), pos(0, 1)),
        vec![vec![5, 6, 7, 1, 2, 3, 4]],
        &[(0, 0, 7, 7)],
    );
}
#[test]
fn do_move_first_ends_before_second_unbalanced() {
    check(
        vec![vec![2, 1, 7, 4, 5, 6, 3]],
        0,
        (pos(0, 2), pos(0, 1)),
        vec![vec![2, 3, 6, 5, 4, 7, 1]],
        &[(0, 0, 7, 7)],
    );
}
#[test]
fn do_move_second_ends_before_first_pinned() {
    check(
        vec![vec![8, 7, 3, 4, 5, 6, 2, 1]],
        1,
        (pos(0, 6), pos(0, 2)),
        vec![vec![8, 2, 3, 4, 5, 6, 7, 1]],
        &[(0, 1, 8, 8)],
    );
}
