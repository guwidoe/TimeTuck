use std::{
    collections::BTreeMap,
    ops::Bound::*,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn bounds_and_double_ended_iteration_match_tree_in_both_representations() {
    for ceiling in [0, usize::MAX] {
        let mut map = ScalingMap::with_config(ScalingMapConfig {
            maximum_array_size: ceiling,
            ..Default::default()
        });
        let mut tree = BTreeMap::new();
        for key in [5, 1, 7, 3] {
            map.get_or_insert_with(key, || key * 2);
            tree.insert(key, key * 2);
        }
        assert_eq!(map.is_array_based(), ceiling != 0);
        assert_eq!(
            map.iter().rev().collect::<Vec<_>>(),
            tree.iter().rev().collect::<Vec<_>>()
        );
        let mut it = map.iter();
        assert_eq!(it.len(), 4);
        assert_eq!(it.next(), Some((&1, &2)));
        assert_eq!(it.next_back(), Some((&7, &14)));
        assert_eq!(it.len(), 2);
        for lo in [
            Unbounded,
            Included(0),
            Included(3),
            Excluded(3),
            Included(8),
        ] {
            for hi in [
                Unbounded,
                Included(0),
                Included(3),
                Excluded(3),
                Included(8),
            ] {
                let expected = catch_unwind(|| tree.range((lo, hi)).collect::<Vec<_>>());
                let actual = catch_unwind(|| map.range((lo, hi)).collect::<Vec<_>>());
                match (expected, actual) {
                    (Ok(e), Ok(a)) => assert_eq!(a, e),
                    (Err(_), Err(_)) => {}
                    _ => panic!("range disagreement"),
                }
            }
        }
    }
}
#[test]
fn supplier_panics_do_not_promote_or_change_values_and_clear_keeps_tree() {
    let c = ScalingMapConfig {
        array_threshold: 1,
        churn_tolerance: 1,
        maximum_array_size: 2,
    };
    let mut map = ScalingMap::with_config(c);
    map.get_or_insert_with(1, || Box::new(11));
    map.get_or_insert_with(2, || Box::new(22));
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            map.get_or_insert_with(3, || panic!("supplier"));
        }))
        .is_err()
    );
    assert!(map.is_array_based());
    assert_eq!(map.len(), 2);
    assert_eq!(**map.get(&1).unwrap(), 11);
    map.remove(&1);
    map.get_or_insert_with(1, || Box::new(12));
    assert!(!map.is_array_based());
    **map.get_mut(&1).unwrap() = 13;
    assert_eq!(**map.get(&1).unwrap(), 13);
    map.clear();
    assert!(!map.is_array_based());
    assert!(map.is_empty());
    map.get_or_insert_with(9, || Box::new(99));
    assert_eq!(**map.get(&9).unwrap(), 99);
}
#[test]
fn candidate_index_ranges_survive_growth_churn_and_rekeying() {
    let mut index = CandidateIndex::new();
    for id in 0..1100 {
        index.insert(id, id, id * 2).unwrap();
    }
    for id in (0..1100).step_by(2) {
        index.replace(&id, 7, id * 3).unwrap();
    }
    for id in (1..1100).step_by(3) {
        index.remove(&id).unwrap();
    }
    let expected: Vec<_> = (0..1100)
        .filter(|id| id % 3 != 1)
        .filter(|id| {
            if id % 2 == 0 {
                true
            } else {
                (5..10).contains(id)
            }
        })
        .collect();
    let view = index.range(5..10);
    let mut actual: Vec<_> = view.iter().map(|e| e.unwrap().id).collect();
    actual.sort();
    assert_eq!(actual, expected);
    assert_eq!(view.samples(42).count(), actual.len());
    for id in actual {
        assert_eq!(
            index.get(&id).unwrap().value,
            if id % 2 == 0 { id * 3 } else { id * 2 }
        );
    }
}
