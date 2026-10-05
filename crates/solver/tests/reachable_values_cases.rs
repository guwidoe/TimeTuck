// Adapted from pinned Timefold Community ReachableValuesTest (Apache-2.0).
// Stable native owner IDs/value codes replace Java descriptors; see provenance.
use timetuck::*;
fn lists() -> ReachableValues<usize, &'static str> {
    let p = ListProblem::new(
        vec!["V1", "V2", "V3", "V4", "V5"],
        vec![
            ListEntity::new(vec!["V1", "V2"]).with_value_range(vec!["V1", "V2", "V3"]),
            ListEntity::new(vec!["V3"]).with_value_range(vec!["V2", "V3"]),
            ListEntity::new(vec!["V4", "V5"]).with_value_range(vec!["V3", "V4", "V5"]),
        ],
    )
    .unwrap();
    list_reachability(&p)
}
fn basic(ranges: Vec<Vec<&'static str>>, initial: bool) -> ReachableValues<usize, &'static str> {
    let p = FiniteProblem::new(
        ranges
            .into_iter()
            .enumerate()
            .map(|(i, range)| {
                let v = Variable::new(range).allowing_unassigned();
                if initial {
                    v.with_initial(["V1", "V3", "V4"][i])
                } else {
                    v
                }
            })
            .collect(),
    )
    .unwrap();
    basic_reachability(&p)
}
fn values(index: &ReachableValues<usize, &'static str>, origin: &'static str) -> Vec<&'static str> {
    index.co_values(&origin).into_iter().copied().collect()
}
fn expected_standard(index: &ReachableValues<usize, &'static str>) {
    for (origin, expected) in [
        ("V1", vec!["V2", "V3"]),
        ("V2", vec!["V1", "V3"]),
        ("V3", vec!["V1", "V2", "V4", "V5"]),
        ("V4", vec!["V3", "V5"]),
        ("V5", vec!["V3", "V4"]),
    ] {
        assert_eq!(values(index, origin), expected);
    }
    assert!(index.is_value_reachable(Some(&"V1"), Some(&"V2")));
    assert!(index.is_value_reachable(Some(&"V1"), Some(&"V3")));
    assert!(!index.is_value_reachable(Some(&"V1"), Some(&"V5")));
}
#[test]
fn shared_entity_value_range() {
    let p = ListProblem::new(
        vec!["V1", "V2", "V3", "V4"],
        vec![
            ListEntity::new(vec![]).with_value_range(vec!["V1", "V2", "V3"]),
            ListEntity::new(vec![]).with_value_range(vec!["V1", "V2", "V3"]),
            ListEntity::new(vec![]).with_value_range(vec!["V3", "V4"]),
        ],
    )
    .unwrap();
    let index = list_reachability(&p);
    assert_eq!(index.entities(&"V1"), [0, 1]);
    assert_eq!(index.entities(&"V2"), [0, 1]);
    assert_eq!(index.entities(&"V3"), [0, 1, 2]);
    assert_eq!(index.entities(&"V4"), [2]);
    assert!(index.is_entity_reachable(&"V1", &0));
    assert!(index.is_entity_reachable(&"V1", &1));
    assert!(!index.is_entity_reachable(&"V1", &2));
    assert_eq!(values(&index, "V3"), ["V1", "V2", "V4"]);
    assert_eq!(values(&index, "V1"), ["V2", "V3"]);
    assert_eq!(values(&index, "V4"), ["V3"]);
}
#[test]
fn reachable_values_by_value() {
    let index = lists();
    expected_standard(&index);
    assert!(!index.is_value_reachable(Some(&"V1"), None));
}
#[test]
fn unassigned_reachable_values() {
    let index = basic(
        vec![
            vec!["V1", "V2", "V3"],
            vec!["V2", "V3"],
            vec!["V3", "V4", "V5"],
        ],
        true,
    );
    expected_standard(&index);
    assert!(index.is_value_reachable(Some(&"V1"), None));
}
fn sorted(
    index: &ReachableValues<usize, &'static str>,
    origin: &'static str,
    descending: bool,
) -> Vec<&'static str> {
    index
        .co_values_by(&origin, |a, b| if descending { b.cmp(a) } else { a.cmp(b) })
        .into_iter()
        .copied()
        .collect()
}
#[test]
fn sort_ascending_reachable_values() {
    let index = basic(
        vec![
            vec!["V3", "V2", "V1"],
            vec!["V3", "V2"],
            vec!["V5", "V4", "V3", "V2"],
        ],
        false,
    );
    for (origin, expected) in [
        ("V1", vec!["V2", "V3"]),
        ("V2", vec!["V1", "V3", "V4", "V5"]),
        ("V3", vec!["V1", "V2", "V4", "V5"]),
        ("V4", vec!["V2", "V3", "V5"]),
        ("V5", vec!["V2", "V3", "V4"]),
    ] {
        assert_eq!(sorted(&index, origin, false), expected);
    }
}
#[test]
fn sort_descending_reachable_values() {
    let index = basic(
        vec![
            vec!["V2", "V3", "V1"],
            vec!["V2", "V3"],
            vec!["V2", "V3", "V4", "V5"],
        ],
        false,
    );
    for (origin, expected) in [
        ("V1", vec!["V3", "V2"]),
        ("V1", vec!["V3", "V2"]),
        ("V2", vec!["V5", "V4", "V3", "V1"]),
        ("V3", vec!["V5", "V4", "V2", "V1"]),
        ("V4", vec!["V5", "V3", "V2"]),
        ("V5", vec!["V4", "V3", "V2"]),
    ] {
        assert_eq!(sorted(&index, origin, true), expected);
    }
}
// Partial fixture only: Java runtime value/entity type discrimination remains pending.
#[test]
fn reachable_values_by_entity_membership_fixture() {
    let index = lists();
    for (value, expected) in [
        ("V1", vec![0]),
        ("V2", vec![0, 1]),
        ("V3", vec![0, 1, 2]),
        ("V4", vec![2]),
        ("V5", vec![2]),
    ] {
        assert_eq!(index.entities(&value), expected);
    }
    assert!(index.is_entity_reachable(&"V1", &0));
    assert!(!index.is_entity_reachable(&"V1", &1));
    assert!(!index.is_entity_reachable(&"V1", &2));
}
