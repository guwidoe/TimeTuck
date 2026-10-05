// Adapted from pinned Community Int/Long/List/Composite/NullAllowing/Boolean
// ValueRange tests and PlannerAssert helpers. Apache-2.0; see provenance.json.
use timetuck::*;
trait AllValues: ValueRange {
    fn all_values(&self) -> Vec<Self::Value>
    where
        Self: Sized,
    {
        let mut iter = self.iter();
        let values = iter.by_ref().collect();
        assert!(iter.next().is_none());
        values
    }
}
impl<R: ValueRange> AllValues for R {}

fn integer(start: i64, end: i64, step: u64) -> IntegerValueRange {
    IntegerValueRange::new(start, end, step).unwrap()
}
fn listed<V>(values: impl IntoIterator<Item = V>) -> EnumeratedValueRange<V> {
    EnumeratedValueRange::new(values)
}
fn optional<V: Clone + Eq>(
    values: impl IntoIterator<Item = V>,
) -> OptionalValueRange<EnumeratedValueRange<V>> {
    OptionalValueRange::new(listed(values)).unwrap()
}
fn composite<V: Clone + Eq + 'static>(lists: Vec<Vec<V>>) -> CompositeValueRange<'static, V> {
    CompositeValueRange::new(
        lists
            .into_iter()
            .map(|v| Box::new(listed(v)) as Box<dyn ValueRange<Value = V>>)
            .collect(),
    )
    .unwrap()
}
fn draws<R: ValueRange>(r: &R, indices: &[u128]) -> Vec<R::Value> {
    let mut ranks = indices.iter().copied();
    r.random_iter_with(|bound| {
        let n = ranks.next().unwrap();
        assert!(n < bound);
        n
    })
    .take(indices.len())
    .collect()
}
macro_rules! integer_cases {
    ($size:ident,$get:ident,$contains:ident,$iter:ident,$random:ident,$big_end:expr,$big_size:expr) => {
        #[test]
        fn $size() {
            for (start, end, step, n) in [
                (0, 10, 1, 10),
                (100, 120, 1, 20),
                (-15, 25, 1, 40),
                (7, 7, 1, 0),
                (-1000, $big_end, 1, $big_size),
                (0, 10, 2, 5),
                (-1, 9, 2, 5),
                (100, 120, 5, 4),
            ] {
                assert_eq!(integer(start, end, step).len(), n as u128);
            }
        }
        #[test]
        fn $get() {
            for (start, end, step, index, value) in [
                (0, 10, 1, 3, 3),
                (100, 120, 1, 3, 103),
                (-5, 25, 1, 1, -4),
                (-5, 25, 1, 6, 1),
                (-1000, $big_end, 1, 1004, 4),
                (0, 10, 2, 3, 6),
                (-1, 9, 2, 3, 5),
                (100, 120, 5, 3, 115),
            ] {
                assert_eq!(integer(start, end, step).get(index), Some(value));
            }
        }
        #[test]
        fn $contains() {
            for (start, end, step, value, want) in [
                (0, 10, 1, 3, true),
                (0, 10, 1, 10, false),
                (100, 120, 1, 100, true),
                (100, 120, 1, 99, false),
                (-5, 25, 1, -4, true),
                (-5, 25, 1, -20, false),
                (0, 10, 2, 2, true),
                (0, 10, 2, 3, false),
                (-1, 9, 2, 1, true),
                (-1, 9, 2, 2, false),
                (100, 120, 5, 115, true),
                (100, 120, 5, 114, false),
            ] {
                assert_eq!(integer(start, end, step).contains(&value), want);
            }
            assert!(!integer(0, 10, 1).contains_assignment(None));
        }
        #[test]
        fn $iter() {
            for (start, end, step, expected) in [
                (0, 7, 1, vec![0, 1, 2, 3, 4, 5, 6]),
                (100, 104, 1, vec![100, 101, 102, 103]),
                (-4, 3, 1, vec![-4, -3, -2, -1, 0, 1, 2]),
                (7, 7, 1, vec![]),
                (0, 10, 2, vec![0, 2, 4, 6, 8]),
                (-1, 9, 2, vec![-1, 1, 3, 5, 7]),
                (100, 120, 5, vec![100, 105, 110, 115]),
            ] {
                assert_eq!(integer(start, end, step).all_values(), expected);
            }
        }
        #[test]
        fn $random() {
            for (start, end, step, expected) in [
                (0, 7, 1, vec![3, 0]),
                (100, 104, 1, vec![103, 100]),
                (-4, 3, 1, vec![-1, -4]),
                (0, 10, 2, vec![6, 0]),
                (-1, 9, 2, vec![5, -1]),
                (100, 120, 5, vec![115, 100]),
            ] {
                assert_eq!(draws(&integer(start, end, step), &[3, 0]), expected);
            }
            assert_eq!(
                integer(7, 7, 1)
                    .random_iter_with(|_| panic!("empty"))
                    .next(),
                None
            );
        }
    };
}
integer_cases!(
    int_get_size,
    int_get,
    int_contains,
    int_original_iterator,
    int_random_iterator,
    i64::from(i32::MAX) - 100,
    i64::from(i32::MAX) + 900
);
integer_cases!(
    long_get_size,
    long_get,
    long_contains,
    long_original_iterator,
    long_random_iterator,
    i64::MAX - 3000,
    i64::MAX - 2000
);
#[test]
fn list_get_size() {
    for (v, n) in [
        (vec![0, 2, 5, 10], 4),
        (vec![100, 120, 5, 7, 8], 5),
        (vec![-15, 25, 0], 3),
        (vec![], 0),
    ] {
        assert_eq!(listed(v).len(), n);
    }
    assert_eq!(listed(["b", "z", "a"]).len(), 3);
}
#[test]
fn list_get() {
    assert_eq!(listed([0, 2, 5, 10]).get(2), Some(5));
    assert_eq!(listed([100, -120]).get(1), Some(-120));
    assert_eq!(listed(["b", "z", "a", "c", "g", "d"]).get(3), Some("c"));
}
#[test]
fn list_contains() {
    for (v, yes, no) in [
        (vec![0, 2, 5, 10], 5, 4),
        (vec![100, 120, 5, 7, 8], 7, 9),
        (vec![-15, 25, 0], -15, -14),
    ] {
        let r = listed(v);
        assert!(r.contains(&yes));
        assert!(!r.contains(&no));
    }
    assert!(!listed([0, 2, 5, 10]).contains_assignment(None));
    assert!(listed(["b", "z", "a"]).contains(&"a"));
    assert!(!listed(["b", "z", "a"]).contains(&"n"));
}
#[test]
fn list_original_iterator() {
    for v in [
        vec![0, 2, 5, 10],
        vec![100, 120, 5, 7, 8],
        vec![-15, 25, 0],
        vec![],
    ] {
        assert_eq!(listed(v.clone()).all_values(), v);
    }
    assert_eq!(listed(["b", "z", "a"]).all_values(), ["b", "z", "a"]);
}
#[test]
fn list_random_iterator() {
    assert_eq!(draws(&listed([0, 2, 5, 10]), &[2, 0]), [5, 0]);
    assert_eq!(draws(&listed([100, 120, 5, 7, 8]), &[2, 0]), [5, 100]);
    assert_eq!(draws(&listed([-15, 25, 0]), &[2, 0]), [0, -15]);
    assert_eq!(draws(&listed(["b", "z", "a"]), &[2, 0]), ["a", "b"]);
    assert_eq!(listed(Vec::<i32>::new()).random_iter(0).next(), None);
}
fn sorted_variants(r: &impl ValueRange<Value = i32>, asc: &[i32], desc: &[i32]) {
    assert_eq!(r.sorted_by(100, i32::cmp).unwrap().values(), asc);
    let factor = 1;
    let factory_comparator = move |a: &i32, b: &i32| (a * factor).cmp(&(b * factor));
    assert_eq!(r.sorted_by(100, factory_comparator).unwrap().values(), asc);
    assert_eq!(r.sorted_by(100, |a, b| b.cmp(a)).unwrap().values(), desc);
    let factor = -1;
    let factory_comparator = move |a: &i32, b: &i32| (a * factor).cmp(&(b * factor));
    assert_eq!(r.sorted_by(100, factory_comparator).unwrap().values(), desc);
}
#[test]
fn list_sort() {
    sorted_variants(
        &listed([-15, 25, 0, 1, -1]),
        &[-15, -1, 0, 1, 25],
        &[25, 1, 0, -1, -15],
    );
}
#[test]
fn composite_get_size() {
    assert_eq!(
        composite(vec![vec![0, 2, 5, 10], vec![-15, 25, -1]]).len(),
        7
    );
    assert_eq!(
        composite(vec![vec!["a", "b"], vec!["c"], vec!["d"]]).len(),
        4
    );
    assert_eq!(composite::<i32>(vec![vec![]]).len(), 0);
}
#[test]
fn composite_get() {
    let r = composite(vec![vec![0, 2, 5, 10], vec![-15, 25, -1]]);
    assert_eq!(r.get(2), Some(5));
    assert_eq!(r.get(4), Some(-15));
    assert_eq!(r.get(6), Some(-1));
    assert_eq!(
        composite(vec![vec!["a", "b"], vec!["c"], vec!["d"]]).get(2),
        Some("c")
    );
}
#[test]
fn composite_contains() {
    let r = composite(vec![vec![0, 2, 5, 10], vec![-15, 25, -1]]);
    for (yes, no) in [(5, 4), (-15, -14), (-1, 1)] {
        assert!(r.contains(&yes));
        assert!(!r.contains(&no));
    }
    let r = composite(vec![vec!["a", "b"], vec!["c"], vec!["d"]]);
    assert!(r.contains(&"c"));
    assert!(!r.contains(&"n"));
}
#[test]
fn composite_original_iterator() {
    assert_eq!(
        composite(vec![vec![0, 2, 5, 10], vec![-15, 25, -1]]).all_values(),
        [0, 2, 5, 10, -15, 25, -1]
    );
    assert_eq!(
        composite(vec![vec!["a", "b"], vec!["c"], vec!["d"]]).all_values(),
        ["a", "b", "c", "d"]
    );
    assert_eq!(composite::<i32>(vec![vec![]]).iter().next(), None);
}
#[test]
fn composite_random_iterator() {
    assert_eq!(
        draws(
            &composite(vec![vec![0, 2, 5, 10], vec![-15, 25, -1]]),
            &[3, 0]
        ),
        [10, 0]
    );
    assert_eq!(
        draws(
            &composite(vec![vec!["a", "b"], vec!["c"], vec!["d"]]),
            &[3, 0]
        ),
        ["d", "a"]
    );
    assert_eq!(
        composite::<i32>(vec![vec![]])
            .random_iter_with(|_| 0)
            .next(),
        None
    );
}
#[test]
fn composite_sort() {
    sorted_variants(
        &composite(vec![vec![0, 2, 5, 10], vec![-15, 25, -1]]),
        &[-15, -1, 0, 2, 5, 10, 25],
        &[25, 10, 5, 2, 0, -1, -15],
    );
}
#[test]
fn optional_get_size() {
    for (v, n) in [
        (vec![0, 2, 5, 10], 5),
        (vec![100, 120, 5, 7, 8], 6),
        (vec![-15, 25, 0], 4),
        (vec![], 1),
    ] {
        assert_eq!(optional(v).len(), n);
    }
    assert_eq!(optional(["b", "z", "a"]).len(), 4);
}
#[test]
fn optional_get() {
    let r = optional([0, 2, 5, 10]);
    assert_eq!(r.get(0), Some(None));
    assert_eq!(r.get(2), Some(Some(2)));
    let r = optional(["b", "z", "a", "c", "g", "d"]);
    assert_eq!(r.get(3), Some(Some("a")));
    assert_eq!(r.get(6), Some(Some("d")));
}
#[test]
fn optional_contains() {
    let r = optional([0, 2, 5, 10]);
    assert!(r.contains(&Some(5)));
    assert!(!r.contains(&Some(4)));
    assert!(r.contains(&None));
    let r = optional(["b", "z", "a"]);
    assert!(r.contains(&Some("a")));
    assert!(!r.contains(&Some("n")));
    assert!(r.contains(&None));
}
#[test]
fn optional_original_iterator() {
    for v in [
        vec![0, 2, 5, 10],
        vec![100, 120, 5, 7, 8],
        vec![-15, 25, 0],
        vec![],
    ] {
        let expected: Vec<_> = std::iter::once(None)
            .chain(v.iter().copied().map(Some))
            .collect();
        assert_eq!(optional(v).all_values(), expected);
    }
    assert_eq!(
        optional(["b", "z", "a"]).all_values(),
        [None, Some("b"), Some("z"), Some("a")]
    );
}
#[test]
fn optional_random_iterator() {
    assert_eq!(
        draws(&optional([0, 2, 5]), &[3, 2, 1, 0]),
        [Some(5), Some(2), Some(0), None]
    );
    assert_eq!(draws(&optional(Vec::<String>::new()), &[0]), [None]);
}
#[test]
fn optional_sort() {
    let r = optional([-15, 25, 0, 1, -1]);
    let asc = [None, Some(-15), Some(-1), Some(0), Some(1), Some(25)];
    let desc = [None, Some(25), Some(1), Some(0), Some(-1), Some(-15)];
    assert_eq!(r.sorted_present_by(6, i32::cmp).unwrap().all_values(), asc);
    let factor = 1;
    assert_eq!(
        r.sorted_present_by(6, move |a, b| (a * factor).cmp(&(b * factor)))
            .unwrap()
            .all_values(),
        asc
    );
    assert_eq!(
        r.sorted_present_by(6, |a, b| b.cmp(a))
            .unwrap()
            .all_values(),
        desc
    );
    let factor = -1;
    assert_eq!(
        r.sorted_present_by(6, move |a, b| (a * factor).cmp(&(b * factor)))
            .unwrap()
            .all_values(),
        desc
    );
}
#[test]
fn boolean_get_size() {
    assert_eq!(BooleanValueRange.len(), 2);
}
#[test]
fn boolean_get() {
    assert_eq!(BooleanValueRange.get(0), Some(false));
    assert_eq!(BooleanValueRange.get(1), Some(true));
}
#[test]
fn boolean_contains() {
    assert!(BooleanValueRange.contains(&false));
    assert!(BooleanValueRange.contains(&true));
    assert!(!BooleanValueRange.contains_assignment(None));
}
#[test]
fn boolean_original_iterator() {
    assert_eq!(BooleanValueRange.all_values(), [false, true]);
}
#[test]
fn boolean_random_iterator() {
    assert_eq!(
        draws(&BooleanValueRange, &[1, 1, 0, 1]),
        [true, true, false, true]
    );
}
#[test]
fn boolean_get_index_greater_than_size() {
    assert_eq!(BooleanValueRange.get(2), None);
}
