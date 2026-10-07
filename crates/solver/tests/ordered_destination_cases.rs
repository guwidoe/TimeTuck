// Adapted from pinned Timefold Community ElementDestinationSelectorTest and
// PlannerAssert selector/iterator helpers. Apache-2.0; see compat/provenance.json.
use timetuck::*;
fn pos(entity: usize, index: usize) -> Option<ListPosition> {
    Some(ListPosition { entity, index })
}
fn finite(order: &ListDestinationOrder, expected: &[Option<ListPosition>]) {
    assert_eq!(order.len(), expected.len());
    let mut iter = order.iter();
    assert_eq!(iter.len(), expected.len());
    assert_eq!(iter.by_ref().collect::<Vec<_>>(), expected);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
}
fn repeated(
    order: &ListDestinationOrder,
    size: usize,
    ranks: &[usize],
    expected: &[Option<ListPosition>],
) {
    assert_eq!(order.len(), size);
    let mut ranks = ranks.iter().copied().chain(std::iter::repeat(0));
    let mut iter = order.random_iter_with(|bound| {
        assert_eq!(bound, size);
        ranks.next().unwrap()
    });
    assert_eq!(
        iter.by_ref().take(expected.len()).collect::<Vec<_>>(),
        expected
    );
    assert!(iter.next().is_some());
}
#[test]
fn original() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![2, 1]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let order = ListDestinationSelector::new(&p, &a, &[0, 1, 2], &[3, 1, 2], false)
        .unwrap()
        .all();
    finite(
        &order,
        &[
            pos(0, 0),
            pos(1, 0),
            pos(2, 0),
            pos(2, 1),
            pos(0, 2),
            pos(0, 1),
        ],
    );
}
#[test]
fn random() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1, 2]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let order = ListDestinationSelector::new(&p, &a, &[0, 1, 2, 2], &[3, 2, 1], false)
        .unwrap()
        .all();
    repeated(
        &order,
        7,
        &[0, 4, 1, 5, 6, 2],
        &[
            pos(0, 0),
            pos(2, 1),
            pos(1, 0),
            pos(0, 2),
            pos(0, 1),
            pos(2, 0),
        ],
    );
}
#[test]
fn empty_without_entities() {
    let p = ListProblem::new(vec![1, 2, 3], vec![]).unwrap();
    let a = p.initial_assignment();
    let order = ListDestinationSelector::new(&p, &a, &[], &[1, 2, 3], false)
        .unwrap()
        .all();
    finite(&order, &[]);
    let mut random = order.random_iter_with(|_| panic!("empty draw"));
    assert_eq!(random.next(), None);
    assert_eq!(random.next(), None);
}
#[test]
fn starts_of_empty_entities() {
    let p = ListProblem::new(
        vec![1],
        vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
    )
    .unwrap();
    let a = p.initial_assignment();
    let order = ListDestinationSelector::new(&p, &a, &[0, 1], &[], false)
        .unwrap()
        .all();
    finite(&order, &[pos(0, 0), pos(1, 0)]);
    repeated(&order, 2, &[0, 1], &[pos(0, 0)]);
}
#[test]
fn starts_after_pinned_prefixes() {
    let p = ListProblem::new(
        vec![0, 1],
        vec![
            ListEntity::new(vec![]),
            ListEntity::new(vec![0, 1]).with_pinned_prefix(1),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let order = ListDestinationSelector::new(&p, &a, &[0, 1], &[], false)
        .unwrap()
        .all();
    finite(&order, &[pos(0, 0), pos(1, 1)]);
    repeated(&order, 2, &[0, 1], &[pos(0, 0)]);
}
