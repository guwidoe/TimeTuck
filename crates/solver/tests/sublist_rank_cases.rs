// Adapted from pinned Community TriangleElementFactoryTest; Apache-2.0.
// Native span coordinates replace triangle elements; see compat/provenance.json.
use timetuck::*;
fn triangle_coordinates(available: usize, range: std::ops::Range<usize>) -> (u128, usize, usize) {
    let level = available - range.len() + 1;
    let index_on_level = range.start + 1;
    let index = (level as u128 - 1) * level as u128 / 2 + index_on_level as u128;
    (index, level, index_on_level)
}
#[test]
fn triangle_element_value_of() {
    let spans = SubListRankIndex::new(4, 1, 4).unwrap();
    for (index, level, index_on_level) in [
        (1, 1, 1),
        (2, 2, 1),
        (3, 2, 2),
        (4, 3, 1),
        (5, 3, 2),
        (6, 3, 3),
        (7, 4, 1),
        (8, 4, 2),
        (9, 4, 3),
        (10, 4, 4),
    ] {
        let actual = triangle_coordinates(4, spans.at(index - 1).unwrap());
        assert_eq!(actual.0, index);
        assert_eq!(actual.1, level);
        assert_eq!(actual.2, index_on_level);
    }
}
#[test]
fn next_element() {
    let mut count = SubListRankIndex::new(7, 1, 7).unwrap().span_count();
    assert_eq!(count, 28);
    count -= 3;
    count -= 7;
    assert_eq!(count, 18);
    let spans = SubListRankIndex::new(7, 2, 5).unwrap();
    let mut bounds = vec![];
    let first = spans.draw_with(|bound| {
        bounds.push(bound);
        0
    });
    assert_eq!(bounds.pop(), Some(count));
    let first = triangle_coordinates(7, first);
    assert_eq!(first.0, 4);
    assert_eq!(first.1, 3);
    assert_eq!(first.2, 1);
    let last = spans.draw_with(|bound| {
        bounds.push(bound);
        count - 1
    });
    assert_eq!(bounds.pop(), Some(count));
    let last = triangle_coordinates(7, last);
    assert_eq!(last.0, 21);
    assert_eq!(last.1, 6);
    assert_eq!(last.2, 6);
}
#[test]
fn constructor_invalid_bounds() {
    assert!(
        SubListRankIndex::new(7, 5, 4)
            .unwrap_err()
            .to_string()
            .contains("less than or equal to")
    );
    assert!(
        SubListRankIndex::new(7, 0, 4)
            .unwrap_err()
            .to_string()
            .contains("greater than 0")
    );
}
#[test]
fn next_element_invalid_list_size() {
    assert!(matches!(
        SubListRankIndex::new(4, 5, 6),
        Err(SubListRankError::ListTooShort { .. })
    ));
}
#[test]
fn next_element_list_size_far_below_minimum() {
    assert!(
        SubListRankIndex::new(2, 5, 6)
            .unwrap_err()
            .to_string()
            .contains("must be at least the minimum span length")
    );
}
