// Adapted from Timefold Community ListChangeMoveSelectorTest, Apache-2.0.
// Exact source/fixture identities and substitutions: compat/provenance.json.
use timetuck::*;
fn pos(entity: usize, index: usize) -> Option<ListPosition> {
    Some(ListPosition { entity, index })
}
fn render(c: ListValueChange<i32>) -> String {
    let at = |p: Option<ListPosition>| {
        p.map_or_else(
            || "null".into(),
            |p| format!("{}[{}]", char::from(b'A' + p.entity as u8), p.index),
        )
    };
    if c.source().is_none() && c.destination().is_none() {
        "No change".into()
    } else {
        format!(
            "{} {{{}->{}}}",
            c.value(),
            at(c.source()),
            at(c.destination())
        )
    }
}
fn explicit(
    p: ListProblem<i32>,
    values: &[i32],
    destinations: &[Option<ListPosition>],
    expected: &[&str],
) {
    let a = p.initial_assignment();
    let actual: Vec<_> = list_value_changes(
        &p,
        &a,
        values,
        destinations,
        ListChangePairing::Cartesian,
        true,
    )
    .unwrap()
    .map(render)
    .collect();
    assert_eq!(actual, expected);
}
fn anchored(p: ListProblem<i32>, expected: &[&str]) {
    let a = p.initial_assignment();
    let actual: Vec<_> =
        list_anchored_value_changes(&p, &a, p.values(), p.values(), p.allows_unassigned(), true)
            .unwrap()
            .map(render)
            .collect();
    assert_eq!(actual, expected);
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
    explicit(
        p,
        &[3, 1, 2],
        &[
            pos(0, 0),
            pos(1, 0),
            pos(2, 0),
            pos(2, 1),
            pos(0, 2),
            pos(0, 1),
        ],
        &[
            "3 {C[0]->A[0]}",
            "3 {C[0]->B[0]}",
            "3 {C[0]->C[0]}",
            "3 {C[0]->C[1]}",
            "3 {C[0]->A[2]}",
            "3 {C[0]->A[1]}",
            "1 {A[1]->A[0]}",
            "1 {A[1]->B[0]}",
            "1 {A[1]->C[0]}",
            "1 {A[1]->C[1]}",
            "1 {A[1]->A[2]}",
            "1 {A[1]->A[1]}",
            "2 {A[0]->A[0]}",
            "2 {A[0]->B[0]}",
            "2 {A[0]->C[0]}",
            "2 {A[0]->C[1]}",
            "2 {A[0]->A[2]}",
            "2 {A[0]->A[1]}",
        ],
    );
}

#[test]
fn original_with_entity_value_range() {
    let p = ListProblem::from_entity_ranges(vec![
        ListEntity::new(vec![2, 1]).with_value_range(vec![1, 2]),
        ListEntity::new(vec![3]).with_value_range(vec![2, 3]),
    ])
    .unwrap();
    anchored(
        p,
        &[
            "1 {A[1]->A[0]}",
            "1 {A[1]->A[1]}",
            "2 {A[0]->A[0]}",
            "2 {A[0]->B[0]}",
            "2 {A[0]->A[2]}",
            "2 {A[0]->B[1]}",
            "3 {B[0]->B[0]}",
        ],
    );
}

#[test]
fn original_with_pinning() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![
            ListEntity::new(vec![2, 1]).with_pinned_prefix(1),
            ListEntity::new(vec![4]).with_pinned(true),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap();
    explicit(
        p,
        &[4, 3, 1, 2],
        &[pos(2, 0), pos(2, 1), pos(0, 2), pos(0, 1)],
        &[
            "3 {C[0]->C[0]}",
            "3 {C[0]->C[1]}",
            "3 {C[0]->A[2]}",
            "3 {C[0]->A[1]}",
            "1 {A[1]->C[0]}",
            "1 {A[1]->C[1]}",
            "1 {A[1]->A[2]}",
            "1 {A[1]->A[1]}",
        ],
    );
}

#[test]
fn original_with_pinning_and_entity_value_range() {
    let p = ListProblem::from_entity_ranges(vec![
        ListEntity::new(vec![2, 1])
            .with_value_range(vec![1, 2, 3])
            .with_pinned_prefix(1),
        ListEntity::new(vec![4])
            .with_value_range(vec![2, 4])
            .with_pinned(true),
        ListEntity::new(vec![3]).with_value_range(vec![3, 4]),
    ])
    .unwrap();
    anchored(
        p,
        &[
            "1 {A[1]->A[1]}",
            "3 {C[0]->A[1]}",
            "3 {C[0]->C[0]}",
            "3 {C[0]->A[2]}",
        ],
    );
}

#[test]
fn original_allows_unassigned_values() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![
            ListEntity::new(vec![2, 1]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    explicit(
        p,
        &[3, 1, 4, 2],
        &[
            pos(0, 0),
            pos(1, 0),
            pos(2, 0),
            pos(2, 1),
            pos(0, 2),
            pos(0, 1),
            None,
        ],
        &[
            "3 {C[0]->A[0]}",
            "3 {C[0]->B[0]}",
            "3 {C[0]->C[0]}",
            "3 {C[0]->C[1]}",
            "3 {C[0]->A[2]}",
            "3 {C[0]->A[1]}",
            "3 {C[0]->null}",
            "1 {A[1]->A[0]}",
            "1 {A[1]->B[0]}",
            "1 {A[1]->C[0]}",
            "1 {A[1]->C[1]}",
            "1 {A[1]->A[2]}",
            "1 {A[1]->A[1]}",
            "1 {A[1]->null}",
            "4 {null->A[0]}",
            "4 {null->B[0]}",
            "4 {null->C[0]}",
            "4 {null->C[1]}",
            "4 {null->A[2]}",
            "4 {null->A[1]}",
            "No change",
            "2 {A[0]->A[0]}",
            "2 {A[0]->B[0]}",
            "2 {A[0]->C[0]}",
            "2 {A[0]->C[1]}",
            "2 {A[0]->A[2]}",
            "2 {A[0]->A[1]}",
            "2 {A[0]->null}",
        ],
    );
}

#[test]
fn original_allows_unassigned_values_with_entity_value_range() {
    let p = ListProblem::from_entity_ranges(vec![
        ListEntity::new(vec![2, 1]).with_value_range(vec![1, 2]),
        ListEntity::new(vec![3]).with_value_range(vec![2, 3]),
    ])
    .unwrap()
    .allowing_unassigned();
    anchored(
        p,
        &[
            "1 {A[1]->A[0]}",
            "1 {A[1]->A[1]}",
            "1 {A[1]->null}",
            "2 {A[0]->A[0]}",
            "2 {A[0]->B[0]}",
            "2 {A[0]->A[2]}",
            "2 {A[0]->B[1]}",
            "2 {A[0]->null}",
            "3 {B[0]->B[0]}",
            "3 {B[0]->null}",
        ],
    );
}
