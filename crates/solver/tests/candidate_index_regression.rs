use std::collections::BTreeMap;
use timetuck::*;
#[test]
fn updates_and_swap_removals_match_identity_map() {
    let mut index = CandidateIndex::new();
    let mut model = BTreeMap::new();
    for step in 0usize..2000 {
        let id = (step * 17 + step / 11) % 31;
        let key = (step / 3) % 7;
        if step % 4 == 0 {
            let expected = model.remove(&id);
            let actual = index.remove(&id);
            match expected {
                Some((_, v)) => assert_eq!(actual.unwrap().value, v),
                None => assert_eq!(actual, Err(CandidateIndexError::UnknownId(id))),
            }
        } else if let Some((_, old)) = model.insert(id, (key, step)) {
            assert_eq!(index.replace(&id, key, step), Ok(old));
        } else {
            index.insert(id, key, step).unwrap();
        }
        assert_eq!(index.len(), model.len());
        if step % 17 == 0 {
            for key in 0..7 {
                let expected: Vec<_> = model
                    .iter()
                    .filter(|(_, v)| v.0 == key)
                    .map(|(id, v)| (*id, v.1))
                    .collect();
                let mut actual: Vec<_> =
                    index.bucket(&key).iter().map(|v| (v.id, v.value)).collect();
                actual.sort();
                assert_eq!(actual, expected);
                let mut sampled: Vec<_> = index
                    .bucket(&key)
                    .unique(step as u64)
                    .map(|v| (v.id, v.value))
                    .collect();
                sampled.sort();
                assert_eq!(sampled, expected);
            }
            for (id, (_, value)) in &model {
                assert_eq!(index.get(id).unwrap().value, *value);
            }
        }
    }
    for (id, (_, value)) in model {
        assert_eq!(index.remove(&id).unwrap().value, value);
    }
    assert!(index.is_empty());
    assert_eq!(index.key_count(), 0);
}
#[test]
fn multi_key_and_range_views_preserve_occurrences_without_payload_clones() {
    struct Payload(u8);
    let mut index = CandidateIndex::new();
    for (id, key, value) in [(0, 2, 7), (1, 2, 7), (2, 4, 8), (3, 6, 9)] {
        index.insert(id, key, Payload(value)).unwrap();
    }
    let view = index.query_many([4, 2, 4, 99]);
    assert_eq!(
        view.iter().map(|v| v.unwrap().id).collect::<Vec<_>>(),
        [2, 0, 1]
    );
    assert_eq!(view.len(), 3);
    let mut sampled = view.samples(12).map(|v| v.unwrap().id).collect::<Vec<_>>();
    sampled.sort();
    assert_eq!(sampled, [0, 1, 2]);
    assert!(std::ptr::eq(view.get(0).unwrap(), index.get(&2).unwrap()));
    let range = index.range(2..6);
    assert_eq!(
        range.iter().map(|v| v.unwrap().value.0).collect::<Vec<_>>(),
        [7, 7, 8]
    );
    assert_eq!(index.range(..=2).len(), 2);
    assert_eq!(index.range(7..).len(), 0);
    assert_eq!(index.bucket(&2).get(1).unwrap().id, 1);
    assert!(index.bucket(&2).get(2).is_none());
}
#[test]
fn failed_mutations_preserve_entries_and_clear_releases_buckets() {
    let mut index = CandidateIndex::new();
    index.insert("a", 0, 10).unwrap();
    index.insert("b", 0, 20).unwrap();
    assert_eq!(
        index.insert("a", 9, 99),
        Err(CandidateIndexError::DuplicateId("a"))
    );
    assert_eq!(
        index.replace(&"missing", 9, 99),
        Err(CandidateIndexError::UnknownId("missing"))
    );
    assert_eq!(index.bucket(&0).len(), 2);
    assert!(index.bucket(&9).is_empty());
    assert_eq!(index.get(&"a").unwrap().value, 10);
    assert_eq!(index.replace(&"a", 0, 11), Ok(10));
    assert_eq!(index.bucket(&0).get(0).unwrap().value, 11);
    assert_eq!(index.replace(&"a", 1, 12), Ok(11));
    assert_eq!(index.bucket(&0).get(0).unwrap().id, "b");
    index.remove(&"b").unwrap();
    assert_eq!(index.key_count(), 1);
    index.clear();
    assert!(index.is_empty());
    assert_eq!(index.key_count(), 0);
    assert!(index.get(&"a").is_none());
    index.insert("a", 2, 30).unwrap();
    assert_eq!(
        index
            .bucket(&2)
            .repeating(7)
            .take(30)
            .map(|v| v.value)
            .sum::<i32>(),
        900
    );
}
#[test]
fn repeated_group_queries_are_passive_when_empty_and_retain_supplier_errors() {
    let calls = std::cell::Cell::new(0);
    let mut query = LazyConcat::<u8>::new();
    assert!(repeating_candidates(&query, 0).next().is_none());
    query
        .push_deferred(1, || {
            calls.set(calls.get() + 1);
            vec![]
        })
        .unwrap();
    let mut samples = repeating_candidates(&query, 0);
    assert_eq!(calls.get(), 0);
    for _ in 0..3 {
        assert_eq!(
            samples.next(),
            Some(Err(LazyConcatError::SupplierLength {
                group: 0,
                expected: 1,
                actual: 0
            }))
        );
    }
    assert_eq!(calls.get(), 1);
}
