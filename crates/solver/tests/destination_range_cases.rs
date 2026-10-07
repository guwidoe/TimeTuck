// Adapted from ElementDestinationSelectorTest.originalWithEntityValueRange;
// pinned Timefold Community, Apache-2.0. See compat/provenance.json.
use timetuck::*;
#[test]
fn original_with_entity_value_range() {
    let p = ListProblem::new(
        (1..=5).collect(),
        vec![
            ListEntity::new(vec![1, 2]).with_value_range(vec![1, 2]),
            ListEntity::new(vec![3]).with_value_range(vec![2, 3]),
            ListEntity::new(vec![4, 5]).with_value_range(vec![3, 4, 5]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let index = ListDestinationIndex::new(&p, &a, false).unwrap();
    for (value, expected) in [
        (1, vec![0]),
        (2, vec![0, 1]),
        (3, vec![1, 2]),
        (4, vec![2]),
        (5, vec![2]),
    ] {
        assert_eq!(
            index
                .for_value(&value)
                .unwrap()
                .entities()
                .collect::<Vec<_>>(),
            expected
        );
    }
    let destinations = index.for_value(&3).unwrap();
    let mut forward = destinations.entities();
    assert_eq!(forward.next(), Some(1));
    assert_eq!(forward.next(), Some(2));
    assert_eq!(forward.next(), None);
    // Native reverse traversal over the same snapshot replaces Java previous().
    let mut backward = destinations.entities().rev();
    assert_eq!(backward.next(), Some(2));
    assert_eq!(backward.next(), Some(1));
    assert_eq!(backward.next(), None);
}
