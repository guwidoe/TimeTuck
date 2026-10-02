// Adapted from pinned Community Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
use timetuck::*;
fn sum<C: ReversibleCollector<V>, V, const N: usize>(
    first: [i64; N],
    second: [i64; N],
    results: [i64; 5],
    make: impl Fn(i64) -> V,
    read: impl Fn(C::Output) -> Decimal,
) {
    let mut c = C::default();
    assert_eq!(read(c.finish()), Decimal::from(0));
    let a = make(first.into_iter().sum());
    let b = make(second.into_iter().sum());
    c.insert(&a);
    assert_eq!(read(c.finish()), Decimal::from(results[0]));
    c.insert(&b);
    assert_eq!(read(c.finish()), Decimal::from(results[1]));
    c.insert(&b);
    assert_eq!(read(c.finish()), Decimal::from(results[2]));
    assert!(c.remove(&b));
    assert_eq!(read(c.finish()), Decimal::from(results[3]));
    assert!(c.remove(&b));
    assert_eq!(read(c.finish()), Decimal::from(results[4]));
    assert!(c.remove(&a));
    assert_eq!(read(c.finish()), Decimal::from(0));
}
fn sum_update<C: ReversibleCollector<V>, V: Clone, const N: usize>(
    make: impl Fn(i64) -> V,
    read: impl Fn(C::Output) -> Decimal,
) {
    let map = |v| {
        let mut tuple: [Option<V>; N] = std::array::from_fn(|_| None);
        tuple[0] = Some(make(v));
        tuple[0].take().unwrap()
    };
    let a = map(1);
    let b = map(10);
    let new = map(4);
    let mut c = C::default();
    c.insert(&a);
    c.insert(&b);
    assert_eq!(read(c.finish()), Decimal::from(11));
    assert!(c.replace(&a, &new));
    assert_eq!(read(c.finish()), Decimal::from(14));
    assert!(c.replace(&new, &new));
    assert_eq!(read(c.finish()), Decimal::from(14));
    assert!(c.remove(&b));
    assert_eq!(read(c.finish()), Decimal::from(4));
    assert!(c.remove(&new));
    assert_eq!(read(c.finish()), Decimal::from(0));
}
fn average<C: ReversibleCollector<V, Output = Option<Decimal>>, V, const N: usize>(
    make: impl Fn(i64) -> V,
) {
    let map = |v| {
        let mut tuple = [0; N];
        tuple[0] = v;
        make(tuple.into_iter().sum())
    };
    let a = map(4);
    let b = map(1);
    let mut c = C::default();
    assert_eq!(c.finish(), None);
    c.insert(&a);
    assert_eq!(c.finish(), Some(Decimal::from(4)));
    c.insert(&b);
    assert_eq!(c.finish(), Some(Decimal::from(2)));
    c.insert(&b);
    assert_eq!(c.finish(), Some(Decimal::from(2)));
    assert!(c.remove(&b));
    assert_eq!(c.finish(), Some(Decimal::from(2)));
    assert!(c.remove(&b));
    assert_eq!(c.finish(), Some(Decimal::from(4)));
    assert!(c.remove(&a));
    assert_eq!(c.finish(), None);
}
fn average_update<C: ReversibleCollector<V, Output = Option<Decimal>>, V, const N: usize>(
    make: impl Fn(i64) -> V,
) {
    let map = |v| {
        let mut tuple = [0; N];
        tuple[0] = v;
        make(tuple.into_iter().sum())
    };
    let a = map(4);
    let b = map(2);
    let new = map(6);
    let mut c = C::default();
    c.insert(&a);
    c.insert(&b);
    assert_eq!(c.finish(), Some(Decimal::from(3)));
    assert!(c.replace(&a, &new));
    assert_eq!(c.finish(), Some(Decimal::from(4)));
    assert!(c.replace(&new, &new));
    assert_eq!(c.finish(), Some(Decimal::from(4)));
    assert!(c.remove(&b));
    assert_eq!(c.finish(), Some(Decimal::from(6)));
    assert!(c.remove(&new));
    assert_eq!(c.finish(), None);
}

#[test]
fn uni_sum_big_integer() {
    sum::<BigIntSumCollector, _, 1>([1], [10], [1, 11, 21, 11, 1], BigInt::from, Decimal::from);
}

#[test]
fn uni_sum_big_integer_update() {
    sum_update::<BigIntSumCollector, _, 1>(BigInt::from, Decimal::from);
}

#[test]
fn uni_average_big_integer() {
    average::<BigIntAverageCollector, _, 1>(BigInt::from);
}

#[test]
fn uni_average_big_integer_update() {
    average_update::<BigIntAverageCollector, _, 1>(BigInt::from);
}

#[test]
fn uni_sum_big_decimal() {
    sum::<DecimalSumCollector, _, 1>([1], [10], [1, 11, 21, 11, 1], Decimal::from, |v| v);
}

#[test]
fn uni_sum_big_decimal_update() {
    sum_update::<DecimalSumCollector, _, 1>(Decimal::from, |v| v);
}

#[test]
fn uni_average_big_decimal() {
    average::<DecimalAverageCollector, _, 1>(Decimal::from);
}

#[test]
fn uni_average_big_decimal_update() {
    average_update::<DecimalAverageCollector, _, 1>(Decimal::from);
}

#[test]
fn bi_sum_big_integer() {
    sum::<BigIntSumCollector, _, 2>(
        [2, 3],
        [4, 5],
        [5, 14, 23, 14, 5],
        BigInt::from,
        Decimal::from,
    );
}

#[test]
fn bi_sum_big_integer_update() {
    sum_update::<BigIntSumCollector, _, 2>(BigInt::from, Decimal::from);
}

#[test]
fn bi_average_big_integer() {
    average::<BigIntAverageCollector, _, 2>(BigInt::from);
}

#[test]
fn bi_average_big_integer_update() {
    average_update::<BigIntAverageCollector, _, 2>(BigInt::from);
}

#[test]
fn bi_sum_big_decimal() {
    sum::<DecimalSumCollector, _, 2>([2, 3], [4, 5], [5, 14, 23, 14, 5], Decimal::from, |v| v);
}

#[test]
fn bi_sum_big_decimal_update() {
    sum_update::<DecimalSumCollector, _, 2>(Decimal::from, |v| v);
}

#[test]
fn bi_average_big_decimal() {
    average::<DecimalAverageCollector, _, 2>(Decimal::from);
}

#[test]
fn bi_average_big_decimal_update() {
    average_update::<DecimalAverageCollector, _, 2>(Decimal::from);
}

#[test]
fn tri_sum_big_integer() {
    sum::<BigIntSumCollector, _, 3>(
        [2, 3, 1],
        [4, 5, 1],
        [6, 16, 26, 16, 6],
        BigInt::from,
        Decimal::from,
    );
}

#[test]
fn tri_sum_big_integer_update() {
    sum_update::<BigIntSumCollector, _, 3>(BigInt::from, Decimal::from);
}

#[test]
fn tri_average_big_integer() {
    average::<BigIntAverageCollector, _, 3>(BigInt::from);
}

#[test]
fn tri_average_big_integer_update() {
    average_update::<BigIntAverageCollector, _, 3>(BigInt::from);
}

#[test]
fn tri_sum_big_decimal() {
    sum::<DecimalSumCollector, _, 3>(
        [2, 3, 1],
        [4, 5, 1],
        [6, 16, 26, 16, 6],
        Decimal::from,
        |v| v,
    );
}

#[test]
fn tri_sum_big_decimal_update() {
    sum_update::<DecimalSumCollector, _, 3>(Decimal::from, |v| v);
}

#[test]
fn tri_average_big_decimal() {
    average::<DecimalAverageCollector, _, 3>(Decimal::from);
}

#[test]
fn tri_average_big_decimal_update() {
    average_update::<DecimalAverageCollector, _, 3>(Decimal::from);
}

#[test]
fn quad_sum_big_integer() {
    sum::<BigIntSumCollector, _, 4>(
        [2, 3, 1, 4],
        [4, 5, 1, 2],
        [10, 22, 34, 22, 10],
        BigInt::from,
        Decimal::from,
    );
}

#[test]
fn quad_sum_big_integer_update() {
    sum_update::<BigIntSumCollector, _, 4>(BigInt::from, Decimal::from);
}

#[test]
fn quad_average_big_integer() {
    average::<BigIntAverageCollector, _, 4>(BigInt::from);
}

#[test]
fn quad_average_big_integer_update() {
    average_update::<BigIntAverageCollector, _, 4>(BigInt::from);
}

#[test]
fn quad_sum_big_decimal() {
    sum::<DecimalSumCollector, _, 4>(
        [2, 3, 1, 4],
        [4, 5, 1, 2],
        [10, 22, 34, 22, 10],
        Decimal::from,
        |v| v,
    );
}

#[test]
fn quad_sum_big_decimal_update() {
    sum_update::<DecimalSumCollector, _, 4>(Decimal::from, |v| v);
}

#[test]
fn quad_average_big_decimal() {
    average::<DecimalAverageCollector, _, 4>(Decimal::from);
}

#[test]
fn quad_average_big_decimal_update() {
    average_update::<DecimalAverageCollector, _, 4>(Decimal::from);
}
