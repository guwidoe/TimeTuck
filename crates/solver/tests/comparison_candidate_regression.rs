use timetuck::*;
#[test]
fn every_relation_and_side_match_direct_scan_at_extreme_bounds() {
    let mut index = CandidateIndex::new();
    let mut keys = [i64::MIN, -1, 0, 0, 1, i64::MAX];
    for (id, &key) in keys.iter().enumerate() {
        index.insert(id, key, key).unwrap();
    }
    for pass in 0..2 {
        for query in [i64::MIN, -2, -1, 0, 1, 2, i64::MAX] {
            for relation in [
                JoinComparison::Equal,
                JoinComparison::LessThan,
                JoinComparison::LessThanOrEqual,
                JoinComparison::GreaterThan,
                JoinComparison::GreaterThanOrEqual,
            ] {
                for side in [CandidateSide::Left, CandidateSide::Right] {
                    let expected: Vec<_> = keys
                        .iter()
                        .enumerate()
                        .filter(|(_, key)| {
                            let (l, r) = if side == CandidateSide::Left {
                                (**key, query)
                            } else {
                                (query, **key)
                            };
                            match relation {
                                JoinComparison::Equal => l == r,
                                JoinComparison::LessThan => l < r,
                                JoinComparison::LessThanOrEqual => l <= r,
                                JoinComparison::GreaterThan => l > r,
                                JoinComparison::GreaterThanOrEqual => l >= r,
                            }
                        })
                        .map(|(id, _)| id)
                        .collect();
                    let v = comparison_candidates(&index, query, relation, side);
                    let mut actual: Vec<_> = v.iter().map(|v| v.unwrap().id).collect();
                    actual.sort();
                    assert_eq!(actual, expected);
                    let mut sampled: Vec<_> = v.samples(7).map(|v| v.unwrap().id).collect();
                    sampled.sort();
                    assert_eq!(sampled, expected);
                }
            }
        }
        if pass == 0 {
            keys[2] = i64::MAX;
            index.replace(&2, i64::MAX, i64::MAX).unwrap();
        }
    }
}
#[test]
fn borrowed_payloads_and_option_keys_need_no_copy_or_sentinel() {
    struct Payload;
    let mut index = CandidateIndex::new();
    index.insert(0, None, Payload).unwrap();
    index.insert(1, Some(1), Payload).unwrap();
    let query = comparison_candidates(
        &index,
        None,
        JoinComparison::GreaterThan,
        CandidateSide::Left,
    );
    assert_eq!(query.len(), 1);
    assert!(std::ptr::eq(query.get(0).unwrap(), index.get(&1).unwrap()));
    let query = comparison_candidates(&index, None, JoinComparison::Equal, CandidateSide::Right);
    assert_eq!(query.get(0).unwrap().id, 0);
}
