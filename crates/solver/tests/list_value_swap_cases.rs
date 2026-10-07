// Adapted from Timefold Community ListSwapMoveSelectorTest, Apache-2.0.
// Source methods, partial random-prefix fixtures and model substitutions are
// recorded in compat/provenance.json. No Java random-generator parity is claimed.
use timetuck::*;
fn render(a: &ListAssignment<i32>, m: ListMove<i32>, prefix: &str) -> String {
    let at = |p: ListPosition| format!("{}[{}]", char::from(b'A' + p.entity as u8), p.index);
    match m {
        ListMove::Swap { left, right } => format!(
            "{prefix}{} {{{}}} <-> {prefix}{} {{{}}}",
            a.lists()[left.entity][left.index],
            at(left),
            a.lists()[right.entity][right.index],
            at(right)
        ),
        ListMove::Composite(parts) if parts.is_empty() => "No change".into(),
        ListMove::Composite(parts) => {
            let [
                ListMove::Unassign { entity, start, end },
                ListMove::Assign { value, destination },
            ] = parts.as_slice()
            else {
                panic!("unexpected replacement")
            };
            assert_eq!(*end, *start + 1);
            assert_eq!(
                *destination,
                ListPosition {
                    entity: *entity,
                    index: *start
                }
            );
            let pos = at(*destination);
            format!(
                "{prefix}{} {{{pos}->null}}+{prefix}{value} {{null->{pos}}}",
                a.lists()[*entity][*start]
            )
        }
        _ => panic!("unexpected swap"),
    }
}
fn check(p: ListProblem<i32>, left: &[i32], right: &[i32], noops: bool, expected: &[&str]) {
    let a = p.initial_assignment();
    let actual: Vec<_> =
        list_value_swap_moves(&p, &a, left, right, ListSwapPairing::Cartesian, noops)
            .unwrap()
            .map(|m| {
                render(
                    &a,
                    m,
                    if expected.iter().any(|s| s.starts_with('v')) {
                        "v"
                    } else {
                        ""
                    },
                )
            })
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
    check(
        p,
        &[3, 1, 2],
        &[3, 1, 2],
        true,
        &[
            "No change",
            "v3 {C[0]} <-> v1 {A[1]}",
            "v3 {C[0]} <-> v2 {A[0]}",
            "v1 {A[1]} <-> v3 {C[0]}",
            "No change",
            "v1 {A[1]} <-> v2 {A[0]}",
            "v2 {A[0]} <-> v3 {C[0]}",
            "v2 {A[0]} <-> v1 {A[1]}",
            "No change",
        ],
    );
}
#[test]
fn original_with_entity_value_range() {
    let p = ListProblem::from_entity_ranges(vec![
        ListEntity::new(vec![2, 1]).with_value_range(vec![1, 2, 3]),
        ListEntity::new(vec![3]).with_value_range(vec![2, 3]),
    ])
    .unwrap();
    check(
        p,
        &[1, 2, 3],
        &[1, 2, 3],
        false,
        &[
            "1 {A[1]} <-> 2 {A[0]}",
            "2 {A[0]} <-> 1 {A[1]}",
            "2 {A[0]} <-> 3 {B[0]}",
            "3 {B[0]} <-> 2 {A[0]}",
        ],
    );
}
#[test]
fn original_with_pinning() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![2, 1]).with_pinned_prefix(1),
            ListEntity::new(vec![]).with_pinned(true),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap();
    check(
        p,
        &[3, 1, 2],
        &[3, 1, 2],
        true,
        &[
            "No change",
            "v3 {C[0]} <-> v1 {A[1]}",
            "v1 {A[1]} <-> v3 {C[0]}",
            "No change",
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
        ListEntity::new(vec![3]).with_value_range(vec![1, 3, 4]),
    ])
    .unwrap();
    check(
        p,
        &[1, 2, 3, 4],
        &[1, 2, 3, 4],
        false,
        &["1 {A[1]} <-> 3 {C[0]}", "3 {C[0]} <-> 1 {A[1]}"],
    );
}
#[test]
fn original_allows_unassigned_values() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![ListEntity::new(vec![2, 1]), ListEntity::new(vec![3])],
    )
    .unwrap()
    .allowing_unassigned();
    check(
        p,
        &[1, 2, 3, 4],
        &[4, 3, 2, 1],
        true,
        &[
            "v1 {A[1]->null}+v4 {null->A[1]}",
            "v1 {A[1]} <-> v3 {B[0]}",
            "v1 {A[1]} <-> v2 {A[0]}",
            "No change",
            "v2 {A[0]->null}+v4 {null->A[0]}",
            "v2 {A[0]} <-> v3 {B[0]}",
            "No change",
            "v2 {A[0]} <-> v1 {A[1]}",
            "v3 {B[0]->null}+v4 {null->B[0]}",
            "No change",
            "v3 {B[0]} <-> v2 {A[0]}",
            "v3 {B[0]} <-> v1 {A[1]}",
            "No change",
            "v3 {B[0]->null}+v4 {null->B[0]}",
            "v2 {A[0]->null}+v4 {null->A[0]}",
            "v1 {A[1]->null}+v4 {null->A[1]}",
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
    check(
        p,
        &[1, 2, 3],
        &[1, 2, 3],
        false,
        &["1 {A[1]} <-> 2 {A[0]}", "2 {A[0]} <-> 1 {A[1]}"],
    );
}
fn prefix(p: ListProblem<i32>, left: &[i32], right: &[i32], expected: &[&str]) {
    let a = p.initial_assignment();
    let mut moves =
        list_value_swap_moves(&p, &a, left, right, ListSwapPairing::Paired, true).unwrap();
    let actual: Vec<_> = moves
        .by_ref()
        .take(expected.len())
        .map(|m| render(&a, m, ""))
        .collect();
    assert_eq!(actual, expected);
    assert!(moves.next().is_some());
}
// These retain source move prefixes and non-exhaustion observations only. The
// Java never-ending selector metadata and its PRNG policy remain unimplemented.
#[test]
fn random_prefix_fixture() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1, 2]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap();
    prefix(
        p,
        &[2, 3, 2, 3, 2, 3, 1, 1, 1, 1],
        &[1, 2, 3, 1, 2, 3, 1, 2, 3, 1],
        &[
            "2 {A[1]} <-> 1 {A[0]}",
            "3 {C[0]} <-> 2 {A[1]}",
            "2 {A[1]} <-> 3 {C[0]}",
            "3 {C[0]} <-> 1 {A[0]}",
            "No change",
            "No change",
            "No change",
            "1 {A[0]} <-> 2 {A[1]}",
            "1 {A[0]} <-> 3 {C[0]}",
        ],
    );
}
#[test]
fn random_pinning_prefix_fixture() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![]).with_pinned(true),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap();
    prefix(
        p,
        &[1, 2, 3, 1, 2, 3, 1, 2, 3],
        &[1, 3, 2, 1, 3, 2, 1, 3, 2],
        &[
            "2 {A[1]} <-> 3 {C[0]}",
            "3 {C[0]} <-> 2 {A[1]}",
            "2 {A[1]} <-> 3 {C[0]}",
            "3 {C[0]} <-> 2 {A[1]}",
            "2 {A[1]} <-> 3 {C[0]}",
        ],
    );
}
#[test]
fn random_unassigned_prefix_fixture() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![ListEntity::new(vec![2, 1]), ListEntity::new(vec![3])],
    )
    .unwrap()
    .allowing_unassigned();
    prefix(
        p,
        &[2, 3, 4, 2, 3, 4, 2, 3, 4, 1, 1, 1, 1],
        &[1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 1, 4],
        &[
            "2 {A[0]} <-> 1 {A[1]}",
            "3 {B[0]} <-> 2 {A[0]}",
            "3 {B[0]->null}+4 {null->B[0]}",
            "2 {A[0]->null}+4 {null->A[0]}",
            "3 {B[0]} <-> 1 {A[1]}",
            "2 {A[0]->null}+4 {null->A[0]}",
            "2 {A[0]} <-> 3 {B[0]}",
            "3 {B[0]->null}+4 {null->B[0]}",
            "1 {A[1]->null}+4 {null->A[1]}",
        ],
    );
}
