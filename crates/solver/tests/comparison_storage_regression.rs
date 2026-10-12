//! Original controls for actual configurable candidate storage and borrowed data.
use std::sync::Arc;
use timetuck::*;
struct Payload(i64);
#[test]
fn policy_changes_real_storage_without_cloning_payloads_or_losing_captured_keys() {
    let config = ScalingMapConfig {
        array_threshold: 2,
        churn_tolerance: 1,
        maximum_array_size: 100,
    };
    let mut index = CandidateIndex::with_config(config);
    let payload = Arc::new(Payload(7));
    index.insert(0, 0, payload.clone()).unwrap();
    index.insert(1, 1, Arc::new(Payload(8))).unwrap();
    index.insert(2, 2, Arc::new(Payload(9))).unwrap();
    assert_eq!(index.storage_config(), config);
    assert!(index.is_array_based());
    index.insert(3, -1, Arc::new(Payload(0))).unwrap();
    index.remove(&3).unwrap();
    index.insert(4, -2, Arc::new(Payload(0))).unwrap();
    assert!(!index.is_array_based());
    index.remove(&4).unwrap();
    assert!(Arc::ptr_eq(&index.get(&0).unwrap().value, &payload));
    assert_eq!(index.get(&0).unwrap().value.0, 7);
    index.replace(&0, 10, payload.clone()).unwrap();
    assert!(index.bucket(&0).is_empty());
    assert!(Arc::ptr_eq(
        &index.bucket(&10).get(0).unwrap().value,
        &payload
    ));
    let q = comparison_candidates(&index, 3, JoinComparison::LessThan, CandidateSide::Left);
    assert_eq!(
        q.iter().map(|v| v.unwrap().id).collect::<Vec<_>>(),
        vec![1, 2]
    );
    drop(q);
    for id in 0..3 {
        index.remove(&id).unwrap();
    }
    assert!(index.is_empty());
    assert!(!index.is_array_based());
}
#[test]
fn ceiling_zero_and_array_default_preserve_bidirectional_sampling_results() {
    let mut tree = CandidateIndex::with_config(ScalingMapConfig {
        maximum_array_size: 0,
        ..Default::default()
    });
    let mut array = CandidateIndex::new();
    for (id, key) in [10, 20, 20, 30].into_iter().enumerate() {
        tree.insert(id, key, id).unwrap();
        array.insert(id, key, id).unwrap();
    }
    assert!(!tree.is_array_based());
    assert!(array.is_array_based());
    for side in [CandidateSide::Left, CandidateSide::Right] {
        for op in [
            JoinComparison::LessThan,
            JoinComparison::LessThanOrEqual,
            JoinComparison::GreaterThan,
            JoinComparison::GreaterThanOrEqual,
            JoinComparison::Equal,
        ] {
            let a = comparison_candidates(&array, 20, op, side);
            let t = comparison_candidates(&tree, 20, op, side);
            assert_eq!(
                a.iter().map(|r| r.unwrap().id).collect::<Vec<_>>(),
                t.iter().map(|r| r.unwrap().id).collect::<Vec<_>>()
            );
            for seed in 0..10 {
                assert_eq!(
                    a.samples(seed).map(|r| r.unwrap().id).collect::<Vec<_>>(),
                    t.samples(seed).map(|r| r.unwrap().id).collect::<Vec<_>>()
                );
            }
        }
    }
}
