//! Original metadata, independent enumeration and refresh regressions.
use timetuck::*;
fn enumerated(n: usize, owners: usize, optional: bool) -> usize {
    fn visit(value: usize, n: usize, optional: bool, rows: &mut [Vec<usize>]) -> usize {
        if value == n {
            return 1;
        }
        let mut total = if optional {
            visit(value + 1, n, optional, rows)
        } else {
            0
        };
        for owner in 0..rows.len() {
            for index in 0..=rows[owner].len() {
                rows[owner].insert(index, value);
                total += visit(value + 1, n, optional, rows);
                rows[owner].remove(index);
            }
        }
        total
    }
    visit(0, n, optional, &mut vec![vec![]; owners])
}
#[test]
fn unrestricted_estimates_match_independent_ordered_assignment_enumeration() {
    for n in 0..=4 {
        for owners in 0..=3 {
            for optional in [false, true] {
                let m = ManagedValueRanges::shared(
                    ManagedRangeKind::List {
                        allows_unassigned: optional,
                    },
                    vec![(0..n).collect()],
                    owners,
                );
                let stats = m.statistics().unwrap();
                let count = enumerated(n, owners, optional);
                if count == 0 {
                    assert!(stats.has_empty_required_range);
                    assert_eq!(stats.scale.log10(), 0.0);
                } else {
                    assert!(!stats.has_empty_required_range);
                    assert!((10f64.powf(stats.scale.log10()) - count as f64).abs() < 1e-8);
                }
            }
        }
    }
    let large = ManagedValueRanges::shared(
        ManagedRangeKind::List {
            allows_unassigned: true,
        },
        vec![(0..500).collect()],
        20,
    );
    assert!(large.statistics().unwrap().scale.log10().is_finite());
}
#[test]
fn range_estimates_are_not_claimed_as_exact_and_shared_counts_stay_distinct() {
    let mut m = ManagedValueRanges::entity_provided(
        ManagedRangeKind::List {
            allows_unassigned: false,
        },
        vec![vec![vec![1]], vec![vec![2]]],
    );
    // Only one real assignment; independent eligibility estimates 6 * 1/2 * 1/2.
    let s = m.statistics().unwrap();
    assert!((10f64.powf(s.scale.log10()) - 1.5).abs() < 1e-12);
    m.reset_shared(vec![vec![1, 2]], 2);
    assert!(!m.has_entity_providers());
    let s = m.statistics().unwrap();
    assert_eq!(s.approximate_value_count, 2);
    assert!((10f64.powf(s.scale.log10()) - 6.0).abs() < 1e-12);
    m.reset_entity_provided(vec![vec![vec![1, 2]], vec![vec![1, 2]]]);
    assert!(m.has_entity_providers());
    assert_eq!(m.statistics().unwrap().approximate_value_count, 4);
    let other = ManagedValueRanges::shared(
        ManagedRangeKind::Basic {
            allows_unassigned: false,
        },
        vec![vec![0]],
        3,
    )
    .statistics()
    .unwrap();
    assert!(matches!(
        ManagedRangeStatistics::combine_fields(&[s, other]),
        Err(ManagedRangeError::EntityCount { .. })
    ));
    assert_eq!(
        ManagedRangeStatistics::combine_fields(&[])
            .unwrap()
            .variable_count,
        0
    );
    let huge = ManagedRangeStatistics {
        approximate_value_count: usize::MAX,
        ..s
    };
    assert!(matches!(
        ManagedRangeStatistics::combine_fields(&[huge, s]),
        Err(ManagedRangeError::CountOverflow)
    ));
}
#[test]
fn initialization_refresh_validates_new_values_without_mutating_prior_snapshots() {
    let mut m = ManagedValueRanges::shared(
        ManagedRangeKind::Basic {
            allows_unassigned: true,
        },
        vec![vec![1, 2]],
        2,
    );
    let a = Assignment::new(vec![None, Some(1)]);
    let before = m.basic_initialization(&a).unwrap();
    assert_eq!(before.uninitialized_variable_count, 0);
    assert!(
        m.basic_initialization(&Assignment::new(vec![Some(9), None]))
            .is_err()
    );
    assert!(m.basic_initialization(&Assignment::new(vec![])).is_err());
    m.reset_entity_provided(vec![vec![vec![2]], vec![vec![2]]]);
    assert!(m.basic_initialization(&a).is_err());
    assert_eq!(before.genuine_entity_count, 2);
    let m = ManagedValueRanges::shared(
        ManagedRangeKind::List {
            allows_unassigned: true,
        },
        vec![vec![1, 2, 3]],
        2,
    );
    let a = ListAssignment::new(vec![vec![1], vec![]]);
    let stats = m.list_initialization(&a).unwrap();
    assert_eq!(stats.unassigned_value_count, 2);
    assert_eq!(stats.shadow_entity_count, 3);
    assert!(
        m.list_initialization(&ListAssignment::new(vec![vec![1], vec![1]]))
            .is_err()
    );
    assert!(
        m.basic_initialization(&Assignment::new(vec![None, None]))
            .is_err()
    );
}
