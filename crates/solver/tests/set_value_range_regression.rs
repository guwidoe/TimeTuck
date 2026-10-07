use std::{
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;

#[test]
fn ordered_set_operations_match_sequence_reference_exhaustively() {
    let inputs: Vec<Vec<i32>> = (0..4)
        .flat_map(|len| {
            (0..3usize.pow(len)).map(move |mut code| {
                (0..len)
                    .map(|_| {
                        let value = (code % 3) as i32;
                        code /= 3;
                        value
                    })
                    .collect()
            })
        })
        .collect();
    let distinct = |values: &[i32]| {
        let mut out = vec![];
        for &v in values {
            if !out.contains(&v) {
                out.push(v);
            }
        }
        out
    };
    for left in &inputs {
        let a = SetValueRange::new(left.iter().copied());
        let expected = distinct(left);
        assert_eq!(a.values().copied().collect::<Vec<_>>(), expected);
        for (index, value) in expected.iter().enumerate() {
            assert_eq!(a.index_of(value), Some(index as u128));
            assert_eq!(a.get_ref(index as u128), Some(value));
        }
        assert_eq!(a.index_of(&9), None);
        assert_eq!(a.get(u128::MAX), None);
        assert_eq!(a.get(a.len()), None);
        assert_eq!(
            a.iter().rev().collect::<Vec<_>>(),
            expected.iter().rev().copied().collect::<Vec<_>>()
        );
        for right in &inputs {
            let b = SetValueRange::new(right.iter().copied());
            let union = distinct(&[left.as_slice(), right.as_slice()].concat());
            let intersection: Vec<_> = expected
                .iter()
                .copied()
                .filter(|v| right.contains(v))
                .collect();
            let difference: Vec<_> = expected
                .iter()
                .copied()
                .filter(|v| !right.contains(v))
                .collect();
            for (range, want) in [
                (a.union(&b), union),
                (a.intersection(&b), intersection),
                (a.difference(&b), difference),
            ] {
                assert_eq!(range.iter().collect::<Vec<_>>(), want);
                for (i, value) in want.iter().enumerate() {
                    assert_eq!(range.index_of(value), Some(i as u128));
                }
            }
        }
    }
}

// Deliberate hash collisions and identity equality ignoring the attached payload.
// No Clone implementation: all operations below must share payloads.
#[derive(Debug)]
struct Member {
    id: i32,
    label: &'static str,
}
impl PartialEq for Member {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Member {}
impl Hash for Member {
    fn hash<H: Hasher>(&self, h: &mut H) {
        0.hash(h);
    }
}
#[test]
fn sharing_retains_first_representative_and_survives_comparator_panics() {
    let a = SetValueRange::new([
        Member {
            id: 2,
            label: "first",
        },
        Member {
            id: 1,
            label: "one",
        },
        Member {
            id: 2,
            label: "duplicate",
        },
        Member {
            id: 3,
            label: "three",
        },
    ]);
    let b = SetValueRange::new([
        Member {
            id: 2,
            label: "other",
        },
        Member {
            id: 4,
            label: "four",
        },
    ]);
    let held = a.clone();
    let sorted = a.reordered_by(|a, b| (a.id % 2).cmp(&(b.id % 2)));
    assert_eq!(sorted.values().map(|v| v.id).collect::<Vec<_>>(), [2, 1, 3]);
    let reversed = a.reordered_by(|a, b| b.id.cmp(&a.id));
    assert_eq!(
        reversed.values().map(|v| v.id).collect::<Vec<_>>(),
        [3, 2, 1]
    );
    assert!(std::ptr::eq(
        a.get_ref(0).unwrap(),
        reversed.get_ref(1).unwrap()
    ));
    assert_eq!(
        reversed.index_of(&Member {
            id: 2,
            label: "query"
        }),
        Some(1)
    );
    let union = a.union(&b);
    assert_eq!(union.get_ref(0).unwrap().label, "first");
    assert_eq!(a.intersection(&b).get_ref(0).unwrap().label, "first");
    assert_eq!(
        a.difference(&b).values().map(|v| v.id).collect::<Vec<_>>(),
        [1, 3]
    );
    assert!(
        catch_unwind(AssertUnwindSafe(
            || a.reordered_by(|_, _| panic!("comparison"))
        ))
        .is_err()
    );
    assert!(std::ptr::eq(
        a.get_ref(0).unwrap(),
        held.get_ref(0).unwrap()
    ));
    drop(a);
    drop(b);
    drop(held);
    assert_eq!(
        union.values().map(|v| v.label).collect::<Vec<_>>(),
        ["first", "one", "three", "four"]
    );
}
#[test]
fn distinct_sampling_and_combined_domains_feed_exact_solving() {
    let a = SetValueRange::new([3i64, 1, 3, 2, 1]);
    let mut rank = 0;
    assert_eq!(
        a.random_iter_with(|bound| {
            assert_eq!(bound, 3);
            let next = rank;
            rank = (rank + 1) % bound;
            next
        })
        .take(6)
        .collect::<Vec<_>>(),
        [3, 1, 2, 3, 1, 2]
    );
    assert_eq!(
        a.random_iter(74).take(100).collect::<Vec<_>>(),
        a.clone().random_iter(74).take(100).collect::<Vec<_>>()
    );
    assert_eq!(
        SetValueRange::<i64>::new([])
            .random_iter_with(|_| panic!("empty draw"))
            .next(),
        None
    );
    let b = SetValueRange::new([2, 3, 4]);
    let common = a.intersection(&b);
    assert!(matches!(
        variable_from_range(&common, 1),
        Err(ValueRangeError::MaterializationLimit { size: 2, limit: 1 })
    ));
    let p = FiniteProblem::new(vec![
        variable_from_range(&common, 2).unwrap(),
        variable_from_range(&b.difference(&a), 1).unwrap(),
    ])
    .unwrap();
    let result = exhaustive_search(
        &p,
        |a| SimpleScore::new(a.values().iter().flatten().sum()),
        100,
    );
    assert_eq!(result.evaluations, 2);
    let best = result.best.unwrap();
    assert_eq!(best.assignment.values(), &[Some(3), Some(4)]);
    assert_eq!(best.score, SimpleScore::new(7));
    let optional = OptionalValueRange::new(SetValueRange::<i64>::new([])).unwrap();
    assert_eq!(optional.iter().collect::<Vec<_>>(), [None]);
}
