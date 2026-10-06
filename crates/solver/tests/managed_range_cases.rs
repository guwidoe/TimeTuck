// Adapted from Community ValueRangeManagerTest and its basic/list fixtures.
// Apache-2.0; see compat/provenance.json. Explicit providers replace descriptors.
use ManagedRangeTarget::{Entity, Solution};
use std::sync::Arc;
use timetuck::*;
fn generated(start: usize, n: usize) -> Vec<String> {
    (start..start + n)
        .map(|i| format!("Generated Value {i}"))
        .collect()
}
fn fixture(
    list: bool,
    optional: bool,
    entity: bool,
    composite: bool,
    sorting: bool,
) -> ManagedValueRanges<String> {
    let kind = if list {
        ManagedRangeKind::List {
            allows_unassigned: optional,
        }
    } else {
        ManagedRangeKind::Basic {
            allows_unassigned: optional,
        }
    };
    if !entity {
        let n = if sorting {
            if composite { 3 } else { 6 }
        } else {
            2
        };
        let mut providers = vec![generated(0, n)];
        if composite {
            providers.push(generated(n, n));
        }
        ManagedValueRanges::shared(kind, providers, if sorting && !list { 1 } else { 2 })
    } else if sorting {
        let providers = if composite {
            vec![vec![generated(0, 3), generated(3, 3)]; 2]
        } else {
            vec![vec![generated(0, 3)], vec![generated(3, 3)]]
        };
        ManagedValueRanges::entity_provided(kind, providers)
    } else {
        let code = |i| {
            if list || composite || optional {
                format!("v{i}")
            } else {
                format!("{i}")
            }
        };
        let providers = if composite && list {
            vec![
                vec![vec![code(1), code(2)], vec![code(1), code(3)]],
                vec![vec![code(1), code(4)], vec![code(1), code(5)]],
            ]
        } else if composite {
            vec![
                vec![vec![code(1), code(2)], vec![code(1), code(4)]],
                vec![vec![code(1), code(3)], vec![code(1), code(5)]],
            ]
        } else {
            vec![vec![vec![code(1), code(2)]], vec![vec![code(1), code(3)]]]
        };
        ManagedValueRanges::entity_provided(kind, providers)
    }
}
fn extraction(list: bool, optional: bool, entity: bool, composite: bool) {
    let m = fixture(list, optional, entity, composite, false);
    let solution_count = if entity {
        if composite { 5 } else { 3 }
    } else if composite {
        4
    } else {
        2
    };
    assert_eq!(m.raw(Solution).unwrap().len(), solution_count);
    assert_eq!(
        m.get(Solution).unwrap().len(),
        solution_count + u128::from(optional && !list)
    );
    if entity {
        let n = if composite { 3 } else { 2 };
        assert_eq!(m.raw(Entity(0)).unwrap().len(), n);
        assert_eq!(
            m.get(Entity(0)).unwrap().len(),
            n + u128::from(optional && !list)
        );
    }
}
fn sorting_order(
    m: &mut ManagedValueRanges<String>,
    target: ManagedRangeTarget,
    expected: Vec<String>,
) {
    let values = |range: &Arc<EnumeratedValueRange<Option<String>>>| {
        range.values().iter().flatten().cloned().collect::<Vec<_>>()
    };
    let original = m.get(target).unwrap();
    assert_eq!(values(&original), expected);
    let descending = ManagedRangeOrder::new(|a: &String, b: &String| b.cmp(a));
    let sorted = m.sort(target, &descending).unwrap();
    let reverse: Vec<_> = expected.iter().cloned().rev().collect();
    assert_eq!(values(&sorted), reverse);
    assert!(!Arc::ptr_eq(&original, &sorted));
    let other = m.get(target).unwrap();
    assert_eq!(values(&other), reverse);
    assert!(Arc::ptr_eq(&other, &sorted));
    let ascending = ManagedRangeOrder::new(|a: &String, b: &String| a.cmp(b));
    let sorted_ascending = m.sort(target, &ascending).unwrap();
    assert_eq!(values(&sorted_ascending), expected);
    assert!(!Arc::ptr_eq(&other, &sorted_ascending));
    assert!(Arc::ptr_eq(
        &sorted_ascending,
        &m.sort(target, &ascending).unwrap()
    ));
}
fn sorting(list: bool, optional: bool, entity: bool, composite: bool) {
    let mut m = fixture(list, optional, entity, composite, true);
    sorting_order(&mut m, Solution, generated(0, 6));
    if entity {
        // Source helpers construct a fresh manager for each target.
        for i in 0..2 {
            let mut m = fixture(list, optional, entity, composite, true);
            sorting_order(
                &mut m,
                Entity(i),
                if composite {
                    generated(0, 6)
                } else {
                    generated(3 * i, 3)
                },
            );
        }
    }
}
#[test]
fn extract_value_from_solution_assigned_basic_variable() {
    extraction(false, false, false, false);
}
#[test]
fn sort_value_from_solution_assigned_basic_variable() {
    sorting(false, false, false, false);
}
#[test]
fn extract_value_from_solution_unassigned_basic_variable() {
    extraction(false, true, false, false);
}
#[test]
fn sort_value_from_solution_unassigned_basic_variable() {
    sorting(false, true, false, false);
}
#[test]
fn extract_value_from_solution_composite_assigned_basic_variable() {
    extraction(false, false, false, true);
}
#[test]
fn sort_value_from_solution_composite_assigned_basic_variable() {
    sorting(false, false, false, true);
}
#[test]
fn extract_value_from_solution_composite_unassigned_basic_variable() {
    extraction(false, true, false, true);
}
#[test]
fn sort_value_from_solution_composite_unassigned_basic_variable() {
    sorting(false, true, false, true);
}
#[test]
fn extract_value_from_entity_assigned_basic_variable() {
    extraction(false, false, true, false);
}
#[test]
fn sort_value_from_entity_assigned_basic_variable() {
    sorting(false, false, true, false);
}
#[test]
fn extract_value_from_entity_unassigned_basic_variable() {
    extraction(false, true, true, false);
}
#[test]
fn sort_value_from_entity_unassigned_basic_variable() {
    sorting(false, true, true, false);
}
#[test]
fn extract_value_from_entity_composite_assigned_basic_variable() {
    extraction(false, false, true, true);
}
#[test]
fn sort_value_from_entity_composite_assigned_basic_variable() {
    sorting(false, false, true, true);
}
#[test]
fn extract_value_from_entity_composite_unassigned_basic_variable() {
    extraction(false, true, true, true);
}
#[test]
fn sort_value_from_entity_composite_unassigned_basic_variable() {
    sorting(false, true, true, true);
}
#[test]
fn extract_value_from_solution_assigned_list_variable() {
    extraction(true, false, false, false);
}
#[test]
fn sort_value_from_solution_assigned_list_variable() {
    sorting(true, false, false, false);
}
#[test]
fn extract_value_from_solution_unassigned_list_variable() {
    extraction(true, true, false, false);
}
#[test]
fn sort_value_from_solution_unassigned_list_variable() {
    sorting(true, true, false, false);
}
#[test]
fn extract_value_from_solution_composite_assigned_list_variable() {
    extraction(true, false, false, true);
}
#[test]
fn sort_value_from_solution_composite_assigned_list_variable() {
    sorting(true, false, false, true);
}
#[test]
fn extract_value_from_solution_composite_unassigned_list_variable() {
    extraction(true, true, false, true);
}
#[test]
fn sort_value_from_solution_composite_unassigned_list_variable() {
    sorting(true, true, false, true);
}
#[test]
fn extract_value_from_entity_assigned_list_variable() {
    extraction(true, false, true, false);
}
#[test]
fn sort_value_from_entity_assigned_list_variable() {
    sorting(true, false, true, false);
}
#[test]
fn extract_value_from_entity_unassigned_list_variable() {
    extraction(true, true, true, false);
}
#[test]
fn sort_value_from_entity_unassigned_list_variable() {
    sorting(true, true, true, false);
}
#[test]
fn extract_value_from_entity_composite_assigned_list_variable() {
    extraction(true, false, true, true);
}
#[test]
fn sort_value_from_entity_composite_assigned_list_variable() {
    sorting(true, false, true, true);
}
#[test]
fn extract_value_from_entity_composite_unassigned_list_variable() {
    extraction(true, true, true, true);
}
#[test]
fn sort_value_from_entity_composite_unassigned_list_variable() {
    sorting(true, true, true, true);
}

#[test]
fn deduplication_for_solution() {
    let m = ManagedValueRanges::shared(
        ManagedRangeKind::List {
            allows_unassigned: true,
        },
        vec![generated(0, 3)],
        3,
    );
    let first = m.get(Entity(0)).unwrap();
    assert_eq!(
        first.values().iter().flatten().cloned().collect::<Vec<_>>(),
        generated(0, 3)
    );
    assert!(Arc::ptr_eq(&first, &m.get(Entity(1)).unwrap()));
    assert!(Arc::ptr_eq(&first, &m.get(Entity(2)).unwrap()));
}
#[test]
fn deduplication_for_entity() {
    let mut m = ManagedValueRanges::entity_provided(
        ManagedRangeKind::List {
            allows_unassigned: false,
        },
        vec![
            vec![generated(0, 3)],
            vec![generated(0, 3)],
            vec![generated(1, 2)],
        ],
    );
    let first = m.get(Entity(0)).unwrap();
    assert_eq!(
        first.values().iter().flatten().cloned().collect::<Vec<_>>(),
        generated(0, 3)
    );
    assert!(Arc::ptr_eq(&first, &m.get(Entity(1)).unwrap()));
    assert!(!Arc::ptr_eq(&first, &m.get(Entity(2)).unwrap()));
    let descending = ManagedRangeOrder::new(|a: &String, b: &String| b.cmp(a));
    let first = m.sort(Entity(0), &descending).unwrap();
    assert!(Arc::ptr_eq(
        &first,
        &m.sort(Entity(1), &descending).unwrap()
    ));
    assert!(!Arc::ptr_eq(
        &first,
        &m.sort(Entity(2), &descending).unwrap()
    ));
}
