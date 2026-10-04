// Adapted pinned Inner{Bi,Tri,Quad}ConstraintCollectorsTest; Apache-2.0.
// See compat/provenance.json. Date fixtures do not claim Java generic variance/API parity.
use timetuck::*;
fn sequence<C, V>(mut c: C, a: V, b: V, e: [C::Output; 7])
where
    C: ReversibleCollector<V>,
    C::Output: PartialEq + std::fmt::Debug,
{
    assert_eq!(c.finish(), e[0]);
    c.insert(&a);
    assert_eq!(c.finish(), e[1]);
    c.insert(&b);
    assert_eq!(c.finish(), e[2]);
    c.insert(&b);
    assert_eq!(c.finish(), e[3]);
    assert!(c.remove(&b));
    assert_eq!(c.finish(), e[4]);
    assert!(c.remove(&b));
    assert_eq!(c.finish(), e[5]);
    assert!(c.remove(&a));
    assert_eq!(c.finish(), e[6]);
}
fn count<const N: usize>() {
    sequence(
        CountCollector::default(),
        [2, 1, 3, 4][..N].to_vec(),
        [1, 2, 3, 4][..N].to_vec(),
        [0, 1, 2, 3, 2, 1, 0],
    );
}
struct Cardinality;
impl CollectorFinisher<SnapshotMap<i64, ()>> for Cardinality {
    type Output = usize;
    fn finish(v: SnapshotMap<i64, ()>) -> usize {
        v.len()
    }
}
fn distinct<const N: usize>() {
    sequence(
        FinishingCollector::<SetCollector<i64>, Cardinality>::default(),
        [2, 3, 4, 5][..N].iter().sum(),
        [1, 2, 3, 4][..N].iter().sum(),
        [0, 1, 2, 2, 2, 1, 0],
    );
}
fn sum<const N: usize>() {
    let first = [2, 3, 1, 4][..N].iter().sum();
    let second = [4, 5, 1, 2][..N].iter().sum();
    let e = match N {
        2 => [0, 5, 14, 23, 14, 5, 0],
        3 => [0, 6, 16, 26, 16, 6, 0],
        4 => [0, 10, 22, 34, 22, 10, 0],
        _ => unreachable!(),
    };
    sequence(SumCollector::default(), first, second, e);
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct LocalMinute(i32, u8, u8, u8, u8);
fn extrema<const N: usize, const MAX: bool>() {
    let capture = |v| {
        let mut fields = [0; N];
        fields[0] = v;
        let date = LocalMinute(2023, 1, 1, 0, fields.iter().sum::<u8>());
        (date.clone(), date.clone(), date)
    };
    let mut c = ExtremumCollector::<LocalMinute, LocalMinute, LocalMinute, MAX>::default();
    let first = capture(2);
    let second = capture(1);
    let duplicate = if MAX { &first } else { &second };
    let result = |c: &ExtremumCollector<LocalMinute, LocalMinute, LocalMinute, MAX>| {
        c.finish().map(|r| r.value)
    };
    assert_eq!(result(&c), None);
    c.insert(&first);
    assert_eq!(result(&c), Some(LocalMinute(2023, 1, 1, 0, 2)));
    c.insert(&second);
    assert_eq!(
        result(&c),
        Some(LocalMinute(2023, 1, 1, 0, if MAX { 2 } else { 1 }))
    );
    c.insert(duplicate);
    assert_eq!(
        result(&c),
        Some(LocalMinute(2023, 1, 1, 0, if MAX { 2 } else { 1 }))
    );
    assert!(c.remove(duplicate));
    assert_eq!(
        result(&c),
        Some(LocalMinute(2023, 1, 1, 0, if MAX { 2 } else { 1 }))
    );
    assert!(c.remove(duplicate));
    assert_eq!(
        result(&c),
        Some(LocalMinute(2023, 1, 1, 0, if MAX { 1 } else { 2 }))
    );
    assert!(c.remove(if MAX { &second } else { &first }));
    assert_eq!(result(&c), None);
}
#[test]
fn bi_count() {
    count::<2>();
}
#[test]
fn bi_count_distinct() {
    distinct::<2>();
}
#[test]
fn bi_sum() {
    sum::<2>();
}
#[test]
fn bi_min_comparable() {
    extrema::<2, false>();
}
#[test]
fn bi_max_comparable() {
    extrema::<2, true>();
}
#[test]
fn tri_count() {
    count::<3>();
}
#[test]
fn tri_count_distinct() {
    distinct::<3>();
}
#[test]
fn tri_sum() {
    sum::<3>();
}
#[test]
fn tri_min_comparable() {
    extrema::<3, false>();
}
#[test]
fn tri_max_comparable() {
    extrema::<3, true>();
}
#[test]
fn quad_count() {
    count::<4>();
}
#[test]
fn quad_count_distinct() {
    distinct::<4>();
}
#[test]
fn quad_sum() {
    sum::<4>();
}
#[test]
fn quad_min_comparable() {
    extrema::<4, false>();
}
#[test]
fn quad_max_comparable() {
    extrema::<4, true>();
}
