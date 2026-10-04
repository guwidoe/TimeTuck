// Adapted pinned Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest; Apache-2.0.
// See compat/provenance.json. Tuple mappings and all source assertions retained.
use timetuck::*;
struct Value;
impl<T: Clone> CollectorFinisher<Option<Extremum<T, T, T>>> for Value {
    type Output = Option<T>;
    fn finish(v: Option<Extremum<T, T, T>>) -> Option<T> {
        v.map(|r| r.value)
    }
}
type Min<T> = FinishingCollector<MinByCollector<T, T, T>, Value>;
type Max<T> = FinishingCollector<MaxByCollector<T, T, T>, Value>;
type Row = (i64, i64, i64);
type Two = PairCollector<Min<i64>, Max<i64>>;
type TwoRow = (Row, Row);
type Triple = (Option<i64>, Option<i64>, Option<f64>);
struct Flatten3;
impl CollectorFinisher<((Option<i64>, Option<i64>), Option<f64>)> for Flatten3 {
    type Output = Triple;
    fn finish(((a, b), c): ((Option<i64>, Option<i64>), Option<f64>)) -> Triple {
        (a, b, c)
    }
}
type Three = FinishingCollector<PairCollector<Two, AverageCollector>, Flatten3>;
struct Flatten4;
impl CollectorFinisher<(usize, Triple)> for Flatten4 {
    type Output = (usize, Option<i64>, Option<i64>, Option<f64>);
    fn finish((n, (a, b, c)): (usize, Triple)) -> Self::Output {
        (n, a, b, c)
    }
}
type Four = FinishingCollector<PairCollector<CountCollector, Three>, Flatten4>;
struct Tens;
impl CollectorFinisher<usize> for Tens {
    type Output = usize;
    fn finish(n: usize) -> usize {
        n * 10
    }
}
type Then = FinishingCollector<CountCollector, Tens>;
fn mapped<const N: usize>(v: i64) -> i64 {
    let mut t = [0; N];
    t[0] = v;
    t.iter().sum()
}
fn pair<const N: usize>(v: i64) -> TwoRow {
    let v = mapped::<N>(v);
    ((v, v, v), (v, v, v))
}
fn usage<C, V>(mut c: C, first: V, second: V, expected: [C::Output; 7])
where
    C: ReversibleCollector<V>,
    C::Output: PartialEq + std::fmt::Debug,
{
    assert_eq!(c.finish(), expected[0]);
    c.insert(&first);
    assert_eq!(c.finish(), expected[1]);
    c.insert(&second);
    assert_eq!(c.finish(), expected[2]);
    c.insert(&second);
    assert_eq!(c.finish(), expected[3]);
    assert!(c.remove(&second));
    assert_eq!(c.finish(), expected[4]);
    assert!(c.remove(&second));
    assert_eq!(c.finish(), expected[5]);
    assert!(c.remove(&first));
    assert_eq!(c.finish(), expected[6]);
}
fn two<const N: usize>() {
    usage(
        Two::default(),
        pair::<N>(2),
        pair::<N>(1),
        [
            (None, None),
            (Some(2), Some(2)),
            (Some(1), Some(2)),
            (Some(1), Some(2)),
            (Some(1), Some(2)),
            (Some(2), Some(2)),
            (None, None),
        ],
    );
}
fn three<const N: usize>() {
    usage(
        Three::default(),
        (pair::<N>(4), mapped::<N>(4)),
        (pair::<N>(1), mapped::<N>(1)),
        [
            (None, None, None),
            (Some(4), Some(4), Some(4.)),
            (Some(1), Some(4), Some(2.5)),
            (Some(1), Some(4), Some(2.)),
            (Some(1), Some(4), Some(2.5)),
            (Some(4), Some(4), Some(4.)),
            (None, None, None),
        ],
    );
}
fn four<const N: usize>() {
    usage(
        Four::default(),
        (mapped::<N>(4), (pair::<N>(4), mapped::<N>(4))),
        (mapped::<N>(1), (pair::<N>(1), mapped::<N>(1))),
        [
            (0, None, None, None),
            (1, Some(4), Some(4), Some(4.)),
            (2, Some(1), Some(4), Some(2.5)),
            (3, Some(1), Some(4), Some(2.)),
            (2, Some(1), Some(4), Some(2.5)),
            (1, Some(4), Some(4), Some(4.)),
            (0, None, None, None),
        ],
    );
}
fn then<const N: usize>() {
    usage(
        Then::default(),
        mapped::<N>(2),
        mapped::<N>(1),
        [0, 10, 20, 30, 20, 10, 0],
    );
}
fn two_update<const N: usize>() {
    let mut c = Two::default();
    let a = pair::<N>(2);
    let b = pair::<N>(4);
    let d = pair::<N>(3);
    c.insert(&a);
    c.insert(&b);
    assert_eq!(c.finish(), (Some(2), Some(4)));
    assert!(c.replace(&a, &d));
    assert_eq!(c.finish(), (Some(3), Some(4)));
    assert!(c.remove(&b));
    assert!(c.remove(&d));
    assert_eq!(c.finish(), (None, None));
}
fn then_update<const N: usize>() {
    let mut c = Then::default();
    let a = mapped::<N>(1);
    let b = mapped::<N>(99);
    c.insert(&a);
    assert_eq!(<Then as ReversibleCollector<i64>>::finish(&c), 10);
    assert!(c.replace(&a, &b));
    assert_eq!(<Then as ReversibleCollector<i64>>::finish(&c), 10);
    assert!(c.remove(&b));
    assert_eq!(<Then as ReversibleCollector<i64>>::finish(&c), 0);
}
// Native fixture preserves LocalDateTime date/hour/minute equality and ordering
// for the source's fixed 2023-01-01 plus 1..7 minute offsets; no date API claim.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct LocalMinute(i32, u8, u8, u8, u8);
fn extreme_update<T: Clone + Ord + std::fmt::Debug, const MAXIMUM: bool>(map: impl Fn(i64) -> T) {
    type C<T, const M: bool> = FinishingCollector<ExtremumCollector<T, T, T, M>, Value>;
    let mut c = C::<T, MAXIMUM>::default();
    let capture = |v| {
        let v = map(v);
        (v.clone(), v.clone(), v)
    };
    let (a, b, d, e) = if MAXIMUM { (3, 5, 2, 7) } else { (5, 3, 6, 1) };
    let mut x = capture(a);
    let mut y = capture(b);
    c.insert(&x);
    c.insert(&y);
    assert_eq!(c.finish(), Some(map(b)));
    for _ in 0..2 {
        let new = capture(d);
        assert!(c.replace(&y, &new));
        y = new;
        assert_eq!(c.finish(), Some(map(a)));
    }
    let new = capture(e);
    assert!(c.replace(&x, &new));
    x = new;
    assert_eq!(c.finish(), Some(map(e)));
    assert!(c.remove(&y));
    assert_eq!(c.finish(), Some(map(e)));
    assert!(c.remove(&x));
    assert_eq!(c.finish(), None);
}
fn dated<const N: usize, const MAXIMUM: bool>() {
    extreme_update::<_, MAXIMUM>(|v| LocalMinute(2023, 1, 1, 0, mapped::<N>(v) as u8));
}
#[test]
fn uni_compose2() {
    two::<1>();
}
#[test]
fn uni_compose3() {
    three::<1>();
}
#[test]
fn uni_compose4() {
    four::<1>();
}
#[test]
fn uni_collect_and_then() {
    then::<1>();
}
#[test]
fn uni_compose2_update() {
    two_update::<1>();
}
#[test]
fn uni_collect_and_then_update() {
    then_update::<1>();
}
#[test]
fn uni_min_comparable_update() {
    extreme_update::<_, false>(|v| v);
}
#[test]
fn uni_max_comparable_update() {
    extreme_update::<_, true>(|v| v);
}
#[test]
fn bi_compose2() {
    two::<2>();
}
#[test]
fn bi_compose3() {
    three::<2>();
}
#[test]
fn bi_compose4() {
    four::<2>();
}
#[test]
fn bi_collect_and_then() {
    then::<2>();
}
#[test]
fn bi_compose2_update() {
    two_update::<2>();
}
#[test]
fn bi_collect_and_then_update() {
    then_update::<2>();
}
#[test]
fn bi_min_comparable_update() {
    dated::<2, false>();
}
#[test]
fn bi_max_comparable_update() {
    dated::<2, true>();
}
#[test]
fn tri_compose2() {
    two::<3>();
}
#[test]
fn tri_compose3() {
    three::<3>();
}
#[test]
fn tri_compose4() {
    four::<3>();
}
#[test]
fn tri_collect_and_then() {
    then::<3>();
}
#[test]
fn tri_compose2_update() {
    two_update::<3>();
}
#[test]
fn tri_collect_and_then_update() {
    then_update::<3>();
}
#[test]
fn tri_min_comparable_update() {
    dated::<3, false>();
}
#[test]
fn tri_max_comparable_update() {
    dated::<3, true>();
}
#[test]
fn quad_compose2() {
    two::<4>();
}
#[test]
fn quad_compose3() {
    three::<4>();
}
#[test]
fn quad_compose4() {
    four::<4>();
}
#[test]
fn quad_collect_and_then() {
    then::<4>();
}
#[test]
fn quad_compose2_update() {
    two_update::<4>();
}
#[test]
fn quad_collect_and_then_update() {
    then_update::<4>();
}
#[test]
fn quad_min_comparable_update() {
    dated::<4, false>();
}
#[test]
fn quad_max_comparable_update() {
    dated::<4, true>();
}
