// Adapted from pinned Community EqualsFirstReorderTest. Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::hash::{DefaultHasher, Hash, Hasher};
use timetuck::*;
type Shift = (String, i32);
#[test]
fn moves_equal_to_front_carrying_mappings() {
    let employee = JoinProjection::new(|r: &Shift| r.0.clone());
    let start = JoinProjection::new(|r: &Shift| r.1);
    let join = JoinConditions::new([
        JoinCondition::new(JoinComparison::LessThan, &start, &start),
        JoinCondition::new(JoinComparison::Equal, &employee, &employee),
    ]);
    assert_eq!(join.conditions()[0].comparison(), JoinComparison::LessThan);
    assert_eq!(join.conditions()[1].comparison(), JoinComparison::Equal);
    let reordered = join.equals_first();
    assert_eq!(
        reordered.conditions()[0].comparison(),
        JoinComparison::Equal
    );
    assert_eq!(
        reordered.conditions()[1].comparison(),
        JoinComparison::LessThan
    );
    assert!(reordered.conditions()[0].left_projection_is(&employee));
    assert!(reordered.conditions()[1].left_projection_is(&start));
    assert!(reordered.conditions()[0].right_projection_is(&employee));
    assert!(reordered.conditions()[1].right_projection_is(&start));
}
#[test]
fn no_equal_returns_same_instance() {
    let start = JoinProjection::new(|r: &Shift| r.1);
    let less = JoinConditions::new([JoinCondition::new(JoinComparison::LessThan, &start, &start)]);
    assert!(less.same_instance(&less.equals_first()));
    let two = less.and(JoinCondition::new(
        JoinComparison::GreaterThan,
        &start,
        &start,
    ));
    assert!(two.same_instance(&two.equals_first()));
}
#[test]
fn already_equal_first_returns_same_instance() {
    let employee = JoinProjection::new(|r: &Shift| r.0.clone());
    let start = JoinProjection::new(|r: &Shift| r.1);
    let join = JoinConditions::new([
        JoinCondition::new(JoinComparison::Equal, &employee, &employee),
        JoinCondition::new(JoinComparison::LessThan, &start, &start),
    ]);
    assert!(join.same_instance(&join.equals_first()));
}
#[test]
fn equivalent_joins_in_different_order_share_via_equality() {
    let employee = JoinProjection::new(|r: &Shift| r.0.clone());
    let start = JoinProjection::new(|r: &Shift| r.1);
    let eq = JoinCondition::new(JoinComparison::Equal, &employee, &employee);
    let lt = JoinCondition::new(JoinComparison::LessThan, &start, &start);
    let first = JoinConditions::new([eq.clone(), lt.clone()]);
    let last = JoinConditions::new([lt, eq]);
    assert_ne!(first, last);
    let (first, last) = (first.equals_first(), last.equals_first());
    assert_eq!(first, last);
    let mut a = DefaultHasher::new();
    let mut b = DefaultHasher::new();
    first.hash(&mut a);
    last.hash(&mut b);
    assert_eq!(a.finish(), b.finish());
}
