// Adapted from pinned Community ValueRangeManagerTest and attributed fixtures.
// Apache-2.0; see compat/provenance.json. Explicit providers replace descriptors.
use num_bigint::BigUint;
use timetuck::*;
fn basic(values: usize, entities: usize, optional: bool) -> ManagedValueRanges<usize> {
    ManagedValueRanges::shared(
        ManagedRangeKind::Basic {
            allows_unassigned: optional,
        },
        vec![(0..values).collect()],
        entities,
    )
}
fn lists(values: usize, entities: usize, optional: bool) -> ManagedValueRanges<usize> {
    ManagedValueRanges::shared(
        ManagedRangeKind::List {
            allows_unassigned: optional,
        },
        vec![(0..values).collect()],
        entities,
    )
}
fn initialized_lists(values: usize, entities: usize) -> Vec<Vec<usize>> {
    let mut rows = vec![vec![]; entities];
    for i in 0..values {
        rows[i % entities].push(i);
    }
    rows
}
fn check(s: ManagedRangeStatistics, entities: usize, variables: usize, values: usize, log: f64) {
    assert_eq!(s.entity_count, entities);
    assert_eq!(s.variable_count, variables);
    assert_eq!(s.approximate_value_count, values);
    assert!((s.scale.log10() - log).abs() <= log.abs() * 0.01 + 1e-12);
}
#[test]
fn count_entities() {
    let m = lists(10, 3, false);
    let a = ListAssignment::new(initialized_lists(10, 3));
    let stats = m.list_initialization(&a).unwrap();
    assert_eq!(stats.genuine_entity_count, 3);
    assert_eq!(stats.shadow_entity_count, 10);
}
#[test]
fn count_uninitialized_variables() {
    let mut m = basic(10, 3, false);
    let mut values = vec![Some(0), Some(1), Some(2)];
    assert_eq!(
        m.basic_initialization(&Assignment::new(values.clone()))
            .unwrap()
            .uninitialized_variable_count,
        0
    );
    values[0] = None;
    m.reset_shared(vec![(0..10).collect()], 3);
    assert_eq!(
        m.basic_initialization(&Assignment::new(values.clone()))
            .unwrap()
            .uninitialized_variable_count,
        1
    );
    values.fill(None);
    m.reset_shared(vec![(0..10).collect()], 3);
    assert_eq!(
        m.basic_initialization(&Assignment::new(values))
            .unwrap()
            .uninitialized_variable_count,
        3
    );
}
#[test]
fn count_unassigned_values() {
    let mut m = lists(10, 3, false);
    let mut rows = initialized_lists(10, 3);
    assert_eq!(
        m.list_initialization(&ListAssignment::new(rows.clone()))
            .unwrap()
            .unassigned_value_count,
        0
    );
    let missing = rows[0].len();
    assert!(missing > 10 / 3);
    rows[0].clear();
    m.reset_shared(vec![(0..10).collect()], 3);
    assert_eq!(
        m.list_initialization(&ListAssignment::new(rows))
            .unwrap()
            .unassigned_value_count,
        missing
    );
}
#[test]
fn problem_scale_basic() {
    let s = basic(10, 20, false).statistics().unwrap();
    check(s, 20, 20, 10, 20.0);
    assert_eq!(s.scale.log10(), 20.0);
}
#[test]
fn empty_problem_scale() {
    let mut m = basic(27, 27, false);
    m.reset_shared(vec![], 27);
    let s = m.statistics().unwrap();
    check(s, 27, 27, 0, 0.0);
    assert_eq!(s.scale.log10(), 0.0);
}
#[test]
fn empty_problem_scale_allows_unassigned() {
    let mut m = basic(27, 27, true);
    m.reset_shared(vec![], 27);
    let s = m.statistics().unwrap();
    check(s, 27, 27, 1, 0.0);
    assert_eq!(s.scale.log10(), 0.0);
}
#[test]
fn problem_scale_multiple_value_ranges() {
    // The eight distinct fields preserve source values in native textual form.
    let providers = vec![
        vec!["0", "1", "2"],
        vec!["1000", "1001", "1002"],
        vec!["1000000", "1000001", "1000002"],
        vec!["0.00", "0.01", "0.02"],
        vec!["2000-01-01", "2000-01-02", "2000-01-03"],
        vec!["10:00", "10:01", "10:02"],
        vec!["2000-01-01T10:00", "2000-01-01T10:01", "2000-01-01T10:02"],
        vec!["2000", "2001", "2002"],
    ];
    let fields: Vec<_> = providers
        .into_iter()
        .map(|values| {
            ManagedValueRanges::shared(
                ManagedRangeKind::Basic {
                    allows_unassigned: false,
                },
                vec![values],
                1,
            )
            .statistics()
            .unwrap()
        })
        .collect();
    check(
        ManagedRangeStatistics::combine_fields(&fields).unwrap(),
        1,
        8,
        24,
        (3f64.powi(8)).log10(),
    );
}
#[test]
fn basic_variable_problem_scale_entity_providing_value_range() {
    let m = ManagedValueRanges::entity_provided(
        ManagedRangeKind::Basic {
            allows_unassigned: true,
        },
        vec![vec![vec![1, 2]], vec![vec![1, 2, 3]]],
    );
    check(m.statistics().unwrap(), 2, 2, 7, 12f64.log10());
}
#[test]
fn list_variable_problem_scale_entity_providing_value_range() {
    let m = ManagedValueRanges::entity_provided(
        ManagedRangeKind::List {
            allows_unassigned: false,
        },
        vec![vec![vec![1, 2]], vec![vec![1, 2, 3]]],
    );
    let s = m.statistics().unwrap();
    check(s, 2, 2, 5, 12f64.log10());
    assert!((10f64.powf(s.scale.log10()) - 12.0).abs() < 0.12);
}
#[test]
fn problem_scale_single_entity_providing_single_value_range() {
    let m = ManagedValueRanges::entity_provided(
        ManagedRangeKind::Basic {
            allows_unassigned: true,
        },
        vec![vec![vec![1]]],
    );
    check(m.statistics().unwrap(), 1, 1, 2, 2f64.log10());
}
#[test]
fn problem_scale_list() {
    // Exact combinatorial reference follows the permutation/partition behavior
    // specified by MathUtilsTest; no production helper/precision is imported.
    let expected = (20u32..520).fold(BigUint::from(1u32), |a, n| a * n);
    check(
        lists(500, 20, false).statistics().unwrap(),
        20,
        20,
        500,
        ProblemScale::from_count(&expected).log10(),
    );
}
#[test]
fn problem_scale_single_entity_with_assigned_values() {
    let s = lists(1, 1, false).statistics().unwrap();
    check(s, 1, 1, 1, 0.0);
    assert_eq!(s.scale.log10(), 0.0);
}
#[test]
fn problem_scale_single_entity_with_unassigned_values() {
    check(
        lists(1, 1, true).statistics().unwrap(),
        1,
        1,
        1,
        2f64.log10(),
    );
}
