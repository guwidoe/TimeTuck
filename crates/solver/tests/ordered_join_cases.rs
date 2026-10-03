// Adapted from Timefold Community tests; Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::collections::BTreeMap;
use timetuck::{BigInt, Decimal, JoinComparison, JoinNode, TupleChange};

#[test]
fn equal() {
    assert!(JoinComparison::Equal.matches(&Some(1), &Some(1)));
    assert!(!JoinComparison::Equal.matches(&Some(1), &Some(2)));
    assert!(!JoinComparison::Equal.matches(&Some(1), &None));
    assert!(!JoinComparison::Equal.matches(&None, &Some(1)));
}

#[test]
fn less_than() {
    assert!(!JoinComparison::LessThan.matches(&1, &1));
    assert!(JoinComparison::LessThan.matches(&1, &2));
    assert!(!JoinComparison::LessThan.matches(&2, &1));
}

#[test]
fn less_than_or_equal() {
    assert!(JoinComparison::LessThanOrEqual.matches(&1, &1));
    assert!(JoinComparison::LessThanOrEqual.matches(&1, &2));
    assert!(!JoinComparison::LessThanOrEqual.matches(&2, &1));
}

#[test]
fn greater_than() {
    assert!(!JoinComparison::GreaterThan.matches(&1, &1));
    assert!(!JoinComparison::GreaterThan.matches(&1, &2));
    assert!(JoinComparison::GreaterThan.matches(&2, &1));
}

#[test]
fn greater_than_or_equal() {
    assert!(JoinComparison::GreaterThanOrEqual.matches(&1, &1));
    assert!(!JoinComparison::GreaterThanOrEqual.matches(&1, &2));
    assert!(JoinComparison::GreaterThanOrEqual.matches(&2, &1));
}

// Source numeric projections only use small integers and scale-zero decimals.
// Checked conversion deliberately claims no Java longValue overflow/truncation API.
fn projected(comparison: JoinComparison, arity: usize, cases: &[(i64, i64, bool)]) {
    let mut join = JoinNode::comparing(
        comparison,
        |left: &Vec<BigInt>| i64::try_from(left.iter().cloned().sum::<BigInt>()).unwrap(),
        |right: &Decimal| {
            assert_eq!(right.scale(), 0);
            i64::try_from(right.coefficient()).unwrap()
        },
    );
    for &(left, right, expected) in cases {
        let mut values = vec![BigInt::from(0); arity];
        values[0] = BigInt::from(left);
        let decimal = Decimal::new(BigInt::from(right), 0);
        join.apply_left(TupleChange::put(0, values.clone()));
        join.apply_right(TupleChange::put(0, decimal.clone()));
        let actual = join
            .matches()
            .map(|((i, j), (a, b))| ((*i, *j), (a.clone(), b.clone())))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            actual,
            if expected {
                BTreeMap::from([((0, 0), (values, decimal))])
            } else {
                BTreeMap::new()
            }
        );
    }
}

#[test]
fn equal_bi() {
    projected(JoinComparison::Equal, 1, &[(10, 10, true), (1, 0, false)]);
}

#[test]
fn equal_tri() {
    projected(JoinComparison::Equal, 2, &[(10, 10, true), (1, 0, false)]);
}

#[test]
fn equal_quad() {
    projected(JoinComparison::Equal, 3, &[(10, 10, true), (1, 0, false)]);
}

#[test]
fn equal_penta() {
    projected(JoinComparison::Equal, 4, &[(10, 10, true), (1, 0, false)]);
}

#[test]
fn less_than_bi() {
    projected(
        JoinComparison::LessThan,
        1,
        &[(10, 10, false), (10, 1, false), (0, 1, true)],
    );
}

#[test]
fn less_than_tri() {
    projected(
        JoinComparison::LessThan,
        2,
        &[(10, 10, false), (10, 1, false), (0, 1, true)],
    );
}

#[test]
fn less_than_quad() {
    projected(
        JoinComparison::LessThan,
        3,
        &[(10, 10, false), (10, 1, false), (0, 1, true)],
    );
}

#[test]
fn less_than_penta() {
    projected(
        JoinComparison::LessThan,
        4,
        &[(10, 10, false), (10, 1, false), (0, 1, true)],
    );
}

#[test]
fn less_than_or_equal_bi() {
    projected(
        JoinComparison::LessThanOrEqual,
        1,
        &[(10, 10, true), (10, 1, false), (0, 1, true)],
    );
}

#[test]
fn less_than_or_equal_tri() {
    projected(
        JoinComparison::LessThanOrEqual,
        2,
        &[(10, 10, true), (10, 1, false), (0, 1, true)],
    );
}

#[test]
fn less_than_or_equal_quad() {
    projected(
        JoinComparison::LessThanOrEqual,
        3,
        &[(10, 10, true), (10, 1, false), (0, 1, true)],
    );
}

#[test]
fn less_than_or_equal_penta() {
    projected(
        JoinComparison::LessThanOrEqual,
        4,
        &[(10, 10, true), (10, 1, false), (0, 1, true)],
    );
}

#[test]
fn greater_than_bi() {
    projected(
        JoinComparison::GreaterThan,
        1,
        &[(10, 10, false), (10, 1, true), (0, 1, false)],
    );
}

#[test]
fn greater_than_tri() {
    projected(
        JoinComparison::GreaterThan,
        2,
        &[(10, 10, false), (10, 1, true), (0, 1, false)],
    );
}

#[test]
fn greater_than_quad() {
    projected(
        JoinComparison::GreaterThan,
        3,
        &[(10, 10, false), (10, 1, true), (0, 1, false)],
    );
}

#[test]
fn greater_than_penta() {
    projected(
        JoinComparison::GreaterThan,
        4,
        &[(10, 10, false), (10, 1, true), (0, 1, false)],
    );
}

#[test]
fn greater_than_or_equal_bi() {
    projected(
        JoinComparison::GreaterThanOrEqual,
        1,
        &[(10, 10, true), (10, 1, true), (0, 1, false)],
    );
}

#[test]
fn greater_than_or_equal_tri() {
    projected(
        JoinComparison::GreaterThanOrEqual,
        2,
        &[(10, 10, true), (10, 1, true), (0, 1, false)],
    );
}

#[test]
fn greater_than_or_equal_quad() {
    projected(
        JoinComparison::GreaterThanOrEqual,
        3,
        &[(10, 10, true), (10, 1, true), (0, 1, false)],
    );
}

#[test]
fn greater_than_or_equal_penta() {
    projected(
        JoinComparison::GreaterThanOrEqual,
        4,
        &[(10, 10, true), (10, 1, true), (0, 1, false)],
    );
}

#[test]
fn left_bridge_less_than() {
    let mut join = JoinNode::comparing(
        JoinComparison::LessThan,
        |r: &(i32, &str)| r.0,
        |r: &i32| *r,
    );
    for (id, row) in [(0, (30, "age30")), (1, (40, "age40")), (2, (40, "age40b"))] {
        join.apply_left(TupleChange::put(id, row));
    }
    for (query, expected) in [
        (50, vec!["age30", "age40", "age40b"]),
        (40, vec!["age30"]),
        (30, vec![]),
    ] {
        join.apply_right(TupleChange::put(0, query));
        assert_eq!(
            join.matches().map(|(_, (r, _))| r.1).collect::<Vec<_>>(),
            expected
        );
    }
    for (query, expected) in [(50, 3), (40, 1), (30, 0)] {
        join.apply_right(TupleChange::put(0, query));
        assert_eq!(join.matches().count(), expected);
    }
}
#[test]
fn right_bridge_less_than_flips_to_greater_than() {
    let mut join = JoinNode::comparing(
        JoinComparison::LessThan,
        |r: &i32| *r,
        |r: &(i32, &str)| r.0,
    );
    join.apply_right(TupleChange::put(0, (30, "age30")));
    join.apply_right(TupleChange::put(1, (40, "age40")));
    for (query, expected) in [
        (20, vec!["age30", "age40"]),
        (30, vec!["age40"]),
        (40, vec![]),
    ] {
        join.apply_left(TupleChange::put(0, query));
        assert_eq!(
            join.matches().map(|(_, (_, r))| r.1).collect::<Vec<_>>(),
            expected
        );
    }
    for (query, expected) in [(20, 2), (30, 1)] {
        join.apply_left(TupleChange::put(0, query));
        assert_eq!(join.matches().count(), expected);
    }
}
