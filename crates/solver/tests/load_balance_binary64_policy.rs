//! Original explicit numerical-policy regressions, no upstream credit.
use timetuck::{
    Decimal, ReversibleCollector, WrappingLoadBalanceCollector, WrappingLoadBalanceError,
    WrappingLoadBalanceRootPolicy,
};
type C = WrappingLoadBalanceCollector<i32>;
fn binary() -> C {
    C::default().with_root_policy(WrappingLoadBalanceRootPolicy::Binary64)
}
#[test]
fn explicit_binary64_cancels_large_offset_while_exact_default_stays_unchanged() {
    let mut exact = C::default();
    exact.insert(&(0, 1_000_000_000, 0));
    exact.insert(&(1, 1_000_000_001, 0));
    assert_eq!(exact.root_policy(), WrappingLoadBalanceRootPolicy::Exact);
    let saved = exact.finish().unwrap();
    let binary = exact
        .clone()
        .with_root_policy(WrappingLoadBalanceRootPolicy::Binary64);
    assert_eq!(binary.finish().unwrap().unfairness(), &Decimal::ZERO);
    assert_eq!(saved.unfairness(), &Decimal::new(707107.into(), 6));
    assert_eq!(exact.finish().unwrap(), saved);
    assert_eq!(
        binary
            .with_root_policy(WrappingLoadBalanceRootPolicy::Exact)
            .finish()
            .unwrap(),
        saved
    );
}
#[test]
fn wrapping_overflow_shapes_snapshots_and_clone_policy_remain_independent() {
    let mut c = binary();
    c.insert(&(0, i64::MAX, 0));
    c.insert(&(1, 0, 0));
    let saved = c.finish().unwrap();
    assert_eq!(saved.unfairness(), &Decimal::new(707107.into(), 6));
    let mut branch = c.clone();
    assert_eq!(
        branch.root_policy(),
        WrappingLoadBalanceRootPolicy::Binary64
    );
    branch.insert(&(0, i64::MAX, 0));
    let out = branch.finish().unwrap();
    assert_eq!(out.loads()[&0], -2);
    assert_eq!(out.unfairness(), &Decimal::new(141421.into(), 5));
    assert_eq!(c.finish().unwrap(), saved);
    assert!(branch.replace(&(0, i64::MAX, 0), &(1, 0, 0)));
    assert_eq!(branch.finish().unwrap(), saved);
    assert!(!branch.replace(&(9, 0, 0), &(1, 0, 0)));
    assert_eq!(branch.finish().unwrap(), saved);
}
#[test]
fn negative_binary_radicand_is_an_error_and_captures_recover_after_removal() {
    let mut c = binary();
    c.insert(&(0, 3037000500, 0));
    c.insert(&(1, 0, 0));
    assert_eq!(
        c.try_finish(),
        Err(WrappingLoadBalanceError::NegativeRadicand)
    );
    assert!(c.remove(&(0, 3037000500, 0)));
    assert_eq!(c.finish().unwrap().unfairness(), &Decimal::ZERO);
}
