// Adapted Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest.loadBalance, Apache-2.0.
// See compat/provenance.json. Captured native tuples replace Java handles.
use timetuck::*;
fn check<const N: usize>() {
    let mut c = LoadBalanceCollector::<&str>::default();
    let map = |(a, b, _ignored): (&'static str, i64, [i32; N])| (a, b, 0);
    let a = map(("A", 2, [0; N]));
    let b = map(("B", 1, [0; N]));
    let unequal = Decimal::new(707107.into(), 6);
    assert_eq!(c.finish().unfairness(), &Decimal::ZERO);
    c.insert(&a);
    assert_eq!(c.finish().unfairness(), &Decimal::ZERO);
    c.insert(&b);
    assert_eq!(c.finish().unfairness(), &unequal);
    c.insert(&b);
    assert_eq!(c.finish().unfairness(), &Decimal::ZERO);
    assert!(c.remove(&b));
    assert_eq!(c.finish().unfairness(), &unequal);
    assert!(c.remove(&b));
    assert_eq!(c.finish().unfairness(), &Decimal::ZERO);
    assert!(c.remove(&a));
    assert_eq!(c.finish().unfairness(), &Decimal::ZERO);
}
#[test]
fn uni_load_balance() {
    check::<0>();
}
#[test]
fn bi_load_balance() {
    check::<0>();
}
#[test]
fn tri_load_balance() {
    check::<1>();
}
#[test]
fn quad_load_balance() {
    check::<2>();
}
