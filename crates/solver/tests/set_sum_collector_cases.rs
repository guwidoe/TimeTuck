// Adapted pinned Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest; Apache-2.0.
// See compat/provenance.json. Exact tuple mappings and assertion sequences retained.
use timetuck::*;
fn values(c: &SetCollector<i64>) -> Vec<i64> {
    c.finish().keys().copied().collect()
}
fn sets<const N: usize>() {
    let first = if N == 1 {
        2
    } else {
        [2, 1, 3, 4][..N].iter().sum()
    };
    let second = if N == 1 {
        1
    } else {
        [3, 4, 2, 5][..N].iter().sum()
    };
    let expected_first = [0, 2, 3, 6, 10][N];
    let expected_second = [0, 1, 7, 9, 14][N];
    let mut both = vec![expected_first, expected_second];
    both.sort();
    let mut c = SetCollector::default();
    assert_eq!(values(&c), Vec::<i64>::new());
    c.insert(&first);
    assert_eq!(values(&c), vec![expected_first]);
    c.insert(&second);
    assert_eq!(values(&c), both);
    c.insert(&second);
    assert_eq!(values(&c), both);
    assert!(c.remove(&second));
    assert_eq!(values(&c), both);
    assert!(c.remove(&second));
    assert_eq!(values(&c), vec![expected_first]);
    assert!(c.remove(&first));
    assert!(values(&c).is_empty());
}
fn tuple<const N: usize>(v: i64) -> [i64; N] {
    let mut t = [0; N];
    t[0] = v;
    t
}
fn set_update<const N: usize>() {
    let map = |v| tuple::<N>(v)[0];
    let mut c = SetCollector::default();
    let a = map(1);
    let b = map(2);
    c.insert(&a);
    c.insert(&b);
    assert_eq!(values(&c), vec![1, 2]);
    assert!(c.replace(&a, &map(3)));
    assert_eq!(values(&c), vec![2, 3]);
    assert!(c.replace(&map(3), &map(3)));
    assert_eq!(values(&c), vec![2, 3]);
    assert!(c.remove(&b));
    assert_eq!(values(&c), vec![3]);
    assert!(c.remove(&map(3)));
    assert!(values(&c).is_empty());
}
fn count_update<const N: usize>() {
    let mut c = CountCollector::default();
    c.insert(&tuple::<N>(1));
    assert_eq!(
        <CountCollector as ReversibleCollector<[i64; N]>>::finish(&c),
        1
    );
    assert!(c.replace(&tuple::<N>(1), &tuple::<N>(42)));
    assert_eq!(
        <CountCollector as ReversibleCollector<[i64; N]>>::finish(&c),
        1
    );
    assert!(c.remove(&tuple::<N>(42)));
    assert_eq!(
        <CountCollector as ReversibleCollector<[i64; N]>>::finish(&c),
        0
    );
}
fn sum_update<const N: usize>() {
    let map = |v| tuple::<N>(v).iter().sum();
    let mut c = SumCollector::default();
    c.insert(&map(2));
    assert_eq!(c.finish(), 2);
    assert!(c.replace(&map(2), &map(5)));
    assert_eq!(c.finish(), 5);
    assert!(c.replace(&map(5), &map(5)));
    assert_eq!(c.finish(), 5);
    c.insert(&map(3));
    assert_eq!(c.finish(), 8);
    assert!(c.replace(&map(5), &map(1)));
    assert_eq!(c.finish(), 4);
    assert!(c.remove(&map(3)));
    assert_eq!(c.finish(), 1);
    assert!(c.remove(&map(1)));
    assert_eq!(c.finish(), 0);
}
struct Distinct;
impl CollectorFinisher<SnapshotMap<&'static str, ()>> for Distinct {
    type Output = usize;
    fn finish(s: SnapshotMap<&'static str, ()>) -> usize {
        s.len()
    }
}
fn distinct_update<const N: usize>() {
    let map = |v| {
        let mut t = [None; N];
        t[0] = Some(v);
        t[0].unwrap()
    };
    let mut c = FinishingCollector::<SetCollector<&str>, Distinct>::default();
    c.insert(&map("a"));
    c.insert(&map("b"));
    assert_eq!(c.finish(), 2);
    assert!(c.replace(&map("a"), &map("b")));
    assert_eq!(c.finish(), 1);
    assert!(c.replace(&map("b"), &map("b")));
    assert_eq!(c.finish(), 1);
    assert!(c.replace(&map("b"), &map("c")));
    assert_eq!(c.finish(), 2);
    assert!(c.remove(&map("b")));
    assert_eq!(c.finish(), 1);
    assert!(c.remove(&map("c")));
    assert_eq!(c.finish(), 0);
}
struct MergeIntegers;
impl CollectorFinisher<IntSummary> for MergeIntegers {
    type Output = i32;
    fn finish(v: IntSummary) -> i32 {
        v.sum as i32
    }
}
fn merged<const N: usize>() {
    let (rows, expected) = match N {
        1 => {
            let mut counter = 0;
            let rows = [2, 1, 1].map(|key| {
                counter += 1;
                (key, if counter % 2 == 1 { key } else { key + 1 })
            });
            assert_eq!(counter, 3);
            (
                rows,
                [
                    vec![(2, 2)],
                    vec![(1, 2), (2, 2)],
                    vec![(1, 3), (2, 2)],
                    vec![(1, 1), (2, 2)],
                    vec![(2, 2)],
                ],
            )
        }
        2 => (
            [[2, 1], [1, 1], [1, 2]].map(|[a, b]| (a, b)),
            [
                vec![(2, 1)],
                vec![(1, 1), (2, 1)],
                vec![(1, 3), (2, 1)],
                vec![(1, 2), (2, 1)],
                vec![(2, 1)],
            ],
        ),
        3 => (
            [[2, 1, 2], [1, 1, 2], [1, 2, 3]].map(|[a, b, c]| (a, b + c)),
            [
                vec![(2, 3)],
                vec![(1, 3), (2, 3)],
                vec![(1, 8), (2, 3)],
                vec![(1, 5), (2, 3)],
                vec![(2, 3)],
            ],
        ),
        4 => (
            [[2, 1, 3, 2], [1, 3, 3, 2], [1, 3, 3, 1]].map(|[a, b, c, d]| (a + b, c + d)),
            [
                vec![(3, 5)],
                vec![(3, 5), (4, 5)],
                vec![(3, 5), (4, 9)],
                vec![(3, 5), (4, 4)],
                vec![(3, 5)],
            ],
        ),
        _ => unreachable!(),
    };
    type C = MapCollector<i64, i64, FinishingCollector<IntCollector, MergeIntegers>>;
    let mut c = C::default();
    let read = |c: &C| c.finish().iter().map(|(&k, &v)| (k, v)).collect::<Vec<_>>();
    assert!(read(&c).is_empty());
    for i in 0..3 {
        c.insert(&rows[i]);
        assert_eq!(read(&c), expected[i]);
    }
    assert!(c.remove(&rows[1]));
    assert_eq!(read(&c), expected[3]);
    assert!(c.remove(&rows[2]));
    assert_eq!(read(&c), expected[4]);
    assert!(c.remove(&rows[0]));
    assert!(read(&c).is_empty());
}
#[test]
fn uni_to_set() {
    sets::<1>();
}
#[test]
fn uni_to_sorted_set() {
    sets::<1>();
}
#[test]
fn uni_to_sorted_map_merged() {
    merged::<1>();
}
#[test]
fn uni_count_update() {
    count_update::<1>();
}
#[test]
fn uni_sum_update() {
    sum_update::<1>();
}
#[test]
fn uni_count_distinct_update() {
    distinct_update::<1>();
}
#[test]
fn uni_to_set_update() {
    set_update::<1>();
}
#[test]
fn uni_to_sorted_set_update() {
    set_update::<1>();
}
#[test]
fn bi_to_set() {
    sets::<2>();
}
#[test]
fn bi_to_sorted_set() {
    sets::<2>();
}
#[test]
fn bi_to_sorted_map_merged() {
    merged::<2>();
}
#[test]
fn bi_count_update() {
    count_update::<2>();
}
#[test]
fn bi_sum_update() {
    sum_update::<2>();
}
#[test]
fn bi_count_distinct_update() {
    distinct_update::<2>();
}
#[test]
fn bi_to_set_update() {
    set_update::<2>();
}
#[test]
fn bi_to_sorted_set_update() {
    set_update::<2>();
}
#[test]
fn tri_to_set() {
    sets::<3>();
}
#[test]
fn tri_to_sorted_set() {
    sets::<3>();
}
#[test]
fn tri_to_sorted_map_merged() {
    merged::<3>();
}
#[test]
fn tri_count_update() {
    count_update::<3>();
}
#[test]
fn tri_sum_update() {
    sum_update::<3>();
}
#[test]
fn tri_count_distinct_update() {
    distinct_update::<3>();
}
#[test]
fn tri_to_set_update() {
    set_update::<3>();
}
#[test]
fn tri_to_sorted_set_update() {
    set_update::<3>();
}
#[test]
fn quad_to_set() {
    sets::<4>();
}
#[test]
fn quad_to_sorted_set() {
    sets::<4>();
}
#[test]
fn quad_to_sorted_map_merged() {
    merged::<4>();
}
#[test]
fn quad_count_update() {
    count_update::<4>();
}
#[test]
fn quad_sum_update() {
    sum_update::<4>();
}
#[test]
fn quad_count_distinct_update() {
    distinct_update::<4>();
}
#[test]
fn quad_to_set_update() {
    set_update::<4>();
}
#[test]
fn quad_to_sorted_set_update() {
    set_update::<4>();
}
