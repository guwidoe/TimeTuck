// Adapted from Community RandomSubListSelectorTest and PlannerAssert helpers.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::cell::Cell;
use timetuck::*;

fn check(p: ListProblem<i32>, minimum: usize, maximum: usize, expected: &[(usize, usize)]) {
    let selection = SubListSelection::new(
        &p,
        &p.initial_assignment(),
        SubListSize::new(minimum, maximum).unwrap(),
    )
    .unwrap();
    let count = expected.len() as u128;
    assert_eq!(selection.len(), count);
    let requested = Cell::new(0);
    let mut rank = 0;
    let mut iterator = selection.random_iter_with(|bound| {
        requested.set(bound);
        let next = rank;
        rank = (rank + 1) % count;
        next
    });
    for &(start, length) in expected {
        assert_eq!(
            iterator.next(),
            Some(ListRange {
                entity: 0,
                start,
                end: start + length
            })
        );
    }
    assert!(
        iterator.next().is_some(),
        "selection repeats after every distinct span"
    );
    assert_eq!(requested.get(), count);
}
fn problem(values: Vec<i32>, first: ListEntity<i32>) -> ListProblem<i32> {
    ListProblem::new(values, vec![first, ListEntity::new(vec![])]).unwrap()
}
#[test]
fn random_unrestricted() {
    let p = problem(vec![1, 2, 3, 4], ListEntity::new(vec![1, 2, 3, 4]));
    let n = p.entities()[0].initial().len();
    assert_eq!(n * (n + 1) / 2, 10);
    check(
        p,
        1,
        usize::MAX,
        &[
            (0, 4),
            (0, 3),
            (1, 3),
            (0, 2),
            (1, 2),
            (2, 2),
            (0, 1),
            (1, 1),
            (2, 1),
            (3, 1),
        ],
    );
}
#[test]
fn random_with_pinning() {
    let p = problem(
        vec![1, 2, 3, 4],
        ListEntity::new(vec![1, 2, 3, 4]).with_pinned_prefix(1),
    );
    let n = p.entities()[0].initial().len() - p.entities()[0].pinned_prefix();
    assert_eq!(n * (n + 1) / 2, 6);
    check(
        p,
        1,
        usize::MAX,
        &[(1, 3), (1, 2), (2, 2), (1, 1), (2, 1), (3, 1)],
    );
}
#[test]
fn random_allows_unassigned_values() {
    let p = problem(vec![1, 2, 3, 4], ListEntity::new(vec![1, 2, 3])).allowing_unassigned();
    let n = p.entities()[0].initial().len() - p.entities()[0].pinned_prefix();
    assert_eq!(n * (n + 1) / 2, 6);
    check(
        p,
        1,
        usize::MAX,
        &[(0, 3), (0, 2), (1, 2), (0, 1), (1, 1), (2, 1)],
    );
}
#[test]
fn random_with_sub_list_size_bounds() {
    check(
        problem(vec![1, 2, 3, 4, 5], ListEntity::new(vec![1, 2, 3, 4, 5])),
        2,
        3,
        &[(0, 3), (1, 3), (2, 3), (0, 2), (1, 2), (2, 2), (3, 2)],
    );
}
#[test]
fn empty_when_minimum_sub_list_size_greater_than_list_size() {
    let p = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![1, 2, 3])]).unwrap();
    let selection = SubListSelection::new(
        &p,
        &p.initial_assignment(),
        SubListSize::new(4, usize::MAX).unwrap(),
    )
    .unwrap();
    assert_eq!(selection.len(), 0);
    let mut iterator = selection.random_iter_with(|_| panic!("empty repetition has no draws"));
    assert_eq!(iterator.next(), None);
    assert_eq!(iterator.next(), None);
}
