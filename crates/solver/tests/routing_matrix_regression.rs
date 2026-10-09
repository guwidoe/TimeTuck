//! Original routing storage, identity and lookup regressions.
use std::{
    cell::Cell,
    hash::{Hash, Hasher},
    rc::Rc,
};
use timetuck::*;
#[test]
fn directed_missing_zero_and_foreign_handles_remain_distinct() {
    let mut m = DirectedDistanceMatrix::with_capacity(0).unwrap();
    let a = m.register(7).unwrap();
    let b = m.register(8).unwrap();
    assert_eq!(m.register(7).unwrap(), a);
    assert_eq!(m.get_indexed(a, b), Ok(None));
    m.put_indexed(a, b, 0).unwrap();
    m.put_indexed(b, a, -2).unwrap();
    assert_eq!(m.get_indexed(a, b), Ok(Some(0)));
    assert_eq!(m.get_indexed(b, a), Ok(Some(-2)));
    m.reserve_locations(100).unwrap();
    m.reserve_locations(1).unwrap();
    assert_eq!(m.get_indexed(a, b), Ok(Some(0)));
    assert_eq!(m.location(a), Ok(&7));
    let mut other = DirectedDistanceMatrix::with_capacity(0).unwrap();
    let foreign = other.register(7).unwrap();
    assert_eq!(
        m.get_indexed(a, foreign),
        Err(RoutingError::ForeignLocation)
    );
    let mut cached = CachedLocation::new(7);
    cached.bind(&mut m).unwrap();
    assert!(cached.index_for(&other).is_none());
    cached.bind(&mut other).unwrap();
    assert!(cached.index_for(&m).is_none());
}
#[test]
fn indexed_scoring_does_not_hash_location_keys() {
    #[derive(Clone)]
    struct Key(u32, Rc<Cell<usize>>);
    impl PartialEq for Key {
        fn eq(&self, b: &Self) -> bool {
            self.0 == b.0
        }
    }
    impl Eq for Key {}
    impl Hash for Key {
        fn hash<H: Hasher>(&self, h: &mut H) {
            self.1.set(self.1.get() + 1);
            self.0.hash(h);
        }
    }
    let calls = Rc::new(Cell::new(0));
    let mut m = DirectedDistanceMatrix::with_capacity(2).unwrap();
    let a = m.register(Key(0, calls.clone())).unwrap();
    let b = m.register(Key(1, calls.clone())).unwrap();
    m.put_indexed(a, b, 5).unwrap();
    calls.set(0);
    for _ in 0..10000 {
        assert_eq!(m.get_indexed(a, b), Ok(Some(5)));
    }
    assert_eq!(calls.get(), 0);
}
#[test]
fn routes_report_missing_edges_and_overflow() {
    let mut m = DirectedDistanceMatrix::with_capacity(3).unwrap();
    let a = m.register(0).unwrap();
    let b = m.register(1).unwrap();
    let c = m.register(2).unwrap();
    assert_eq!(m.route_cost(a, [], true), Ok(0));
    assert_eq!(
        m.route_cost(a, [b], false),
        Err(RoutingError::MissingDistance)
    );
    m.put_indexed(a, b, 4).unwrap();
    m.put_indexed(b, c, 7).unwrap();
    m.put_indexed(c, a, 3).unwrap();
    assert_eq!(m.route_cost(a, [b, c], true), Ok(14));
    m.put_indexed(b, c, i64::MAX).unwrap();
    assert_eq!(
        m.route_cost(a, [b, c], false),
        Err(RoutingError::DistanceOverflow)
    );
}
#[test]
fn coordinates_validate_finiteness_and_normalize_signed_zero() {
    assert_eq!(GeoLocation::new(-0.0, 0.0), GeoLocation::new(0.0, -0.0));
    assert!(GeoLocation::new(90.0, 180.0).is_ok());
    assert!(GeoLocation::new(-90.0, -180.0).is_ok());
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            GeoLocation::new(bad, 0.0),
            Err(RoutingError::InvalidLatitude(_))
        ));
        assert!(matches!(
            GeoLocation::new(0.0, bad),
            Err(RoutingError::InvalidLongitude(_))
        ));
    }
}
#[test]
fn selected_timeframe_errors_do_not_fall_back_to_other_measures() {
    let mut single = DirectedDistanceMatrix::with_capacity(2).unwrap();
    single.put(&0, &1, 9).unwrap();
    let mut missing_edge = DirectedDistanceMatrix::with_capacity(2).unwrap();
    missing_edge.register(0).unwrap();
    missing_edge.register(1).unwrap();
    let mut set = TimeframeMatrices::new();
    set.set_single(&single);
    set.set_timeframes(vec![Some(&missing_edge), None], |t: &usize| *t);
    assert_eq!(set.get(&0, &1, None), Ok(9));
    assert_eq!(
        set.get(&0, &1, Some(&0)),
        Err(RoutingError::MissingDistance)
    );
    assert_eq!(
        set.get(&0, &1, Some(&1)),
        Err(RoutingError::MissingTimeframe { index: 1 })
    );
}
