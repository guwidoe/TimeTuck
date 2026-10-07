use std::{
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::{CandidateIndexError, MembershipCandidateIndex};

#[test]
fn updates_and_removals_match_a_scan_of_current_rows() {
    let mut index = MembershipCandidateIndex::new();
    let mut rows = BTreeMap::<usize, (Vec<usize>, usize)>::new();
    let mut random = 19_u64;
    for step in 0..2000 {
        random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = (random >> 32) as usize % 29;
        if step % 7 == 0 {
            let expected = rows.remove(&id).map(|(_, value)| value);
            let actual = index.remove(&id).map(|r| {
                assert_eq!(r.id, id);
                r.value
            });
            assert_eq!(actual, expected.ok_or(CandidateIndexError::UnknownId(id)));
        } else {
            let keys: Vec<_> = (0..step % 5)
                .map(|offset| (id + offset + step) % 9)
                .collect();
            let old = rows.insert(id, (keys.clone(), step));
            assert_eq!(index.put(id, keys, step), old.map(|(_, value)| value));
        }
        assert_eq!(index.len(), rows.len());
        assert_eq!(index.is_empty(), rows.is_empty());
        let keys: BTreeSet<_> = rows.values().flat_map(|(keys, _)| keys).collect();
        assert_eq!(index.key_count(), keys.len());
        for id in 0..29 {
            assert_eq!(
                index.get(&id).map(|r| (r.id, r.value)),
                rows.get(&id).map(|(_, value)| (id, *value))
            );
        }
        for query in [vec![], vec![0], vec![1, 1, 3], vec![2, 4, 6, 8, 99]] {
            let expected: Vec<_> = rows
                .iter()
                .filter(|(_, (keys, _))| query.iter().any(|k| keys.contains(k)))
                .map(|(id, (_, value))| (*id, *value))
                .collect();
            let matches = index.query_any(query);
            assert_eq!(matches.len(), expected.len());
            assert_eq!(
                matches.iter().map(|r| (r.id, r.value)).collect::<Vec<_>>(),
                expected
            );
        }
    }
    index.clear();
    assert!(index.is_empty());
    assert_eq!(index.key_count(), 0);
    assert!(index.query_any(0..9).is_empty());
    index.put(0, [1, 2], 10);
    assert_eq!(index.query_any([2]).get(0).unwrap().value, 10);
}

#[test]
fn empty_membership_null_keys_and_nonclone_payloads_remain_distinct() {
    struct Payload(u32);
    let mut index = MembershipCandidateIndex::new();
    index.put(0, [], Payload(7));
    index.put(1, [None, Some("A"), Some("A")], Payload(7));
    index.put(2, [Some("A")], Payload(7));
    assert_eq!(index.len(), 3);
    assert_eq!(index.key_count(), 2);
    let matches = index.query_any([None, None]);
    assert_eq!(matches.len(), 1);
    assert!(std::ptr::eq(
        matches.get(0).unwrap(),
        index.get(&1).unwrap()
    ));
    let matches = index.query_any([Some("A"), None, Some("A")]);
    assert_eq!(
        matches.iter().rev().map(|r| r.id).collect::<Vec<_>>(),
        [2, 1]
    );
    assert!(matches.get(2).is_none());
    assert_eq!(matches.unique(0).count(), 2);
    assert_eq!(index.put(1, [], Payload(8)).unwrap().0, 7);
    assert!(index.query_any([None]).is_empty());
    assert_eq!(index.key_count(), 1);
    assert_eq!(index.remove(&2).unwrap().value.0, 7);
    assert_eq!(index.key_count(), 0);
    assert_eq!(index.len(), 2);
    assert_eq!(index.remove(&1).unwrap().value.0, 8);
    assert_eq!(index.remove(&0).unwrap().value.0, 7);
    assert!(index.is_empty());
}

#[test]
fn interrupted_key_collection_preserves_old_memberships() {
    let mut index = MembershipCandidateIndex::new();
    index.put(1, [10, 20], "old");
    let keys = [30, 40].into_iter().inspect(|&key| {
        if key == 40 {
            panic!("input interrupted")
        }
    });
    assert!(catch_unwind(AssertUnwindSafe(|| index.put(1, keys, "new"))).is_err());
    assert_eq!(index.get(&1).unwrap().value, "old");
    assert_eq!(index.key_count(), 2);
    assert_eq!(index.query_any([10, 20]).len(), 1);
    assert!(index.query_any([30, 40]).is_empty());
}

#[test]
fn sampling_is_independent_of_overlap_and_query_key_order() {
    let mut index = MembershipCandidateIndex::new();
    index.put(0, [1, 2, 3], "same");
    index.put(1, [1], "same");
    index.put(2, [1, 3], "same");
    let plain = index.query_any([1]);
    let overlap = index.query_any([3, 2, 1, 3, 99]);
    for seed in 0..20 {
        assert_eq!(
            plain.unique(seed).map(|r| r.id).collect::<Vec<_>>(),
            overlap.unique(seed).map(|r| r.id).collect::<Vec<_>>()
        );
        assert_eq!(
            plain
                .repeating(seed)
                .take(100)
                .map(|r| r.id)
                .collect::<Vec<_>>(),
            overlap
                .repeating(seed)
                .take(100)
                .map(|r| r.id)
                .collect::<Vec<_>>()
        );
    }
    let empty = index.query_any([]);
    assert!(empty.unique(0).next().is_none());
    let mut repeated = empty.repeating(0);
    assert!(repeated.next().is_none());
    assert!(repeated.next().is_none());
    let singleton = index.query_any([2]);
    assert!(singleton.repeating(0).take(100).all(|r| r.id == 0));
}
