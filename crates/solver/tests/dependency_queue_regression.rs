use std::{collections::BTreeSet, sync::Arc};
use timetuck::*;
#[test]
fn queue_snapshots_partial_drains_and_invalid_ids_preserve_pending_work() {
    let ranks: Arc<[usize]> = vec![usize::MAX, 0, usize::MAX, 1].into();
    let mut q = NodeOrderQueue::new(ranks.clone());
    assert!(std::ptr::eq(q.ranks().as_ptr(), ranks.as_ptr()));
    for node in [2, 0, 3, 1] {
        assert!(q.offer(node).unwrap());
    }
    assert!(!q.offer(3).unwrap());
    assert_eq!(q.len(), 4);
    assert!(q.contains(3));
    assert_eq!(q.peek(), Some(1));
    let mut snapshot = q.clone();
    assert_eq!(
        q.offer(usize::MAX),
        Err(UnknownOrderNode {
            node: usize::MAX,
            node_count: 4
        })
    );
    assert_eq!(q.len(), 4);
    {
        let mut partial = q.drain();
        assert_eq!(partial.next(), Some(1));
    }
    assert!(!q.contains(1));
    assert_eq!(q.peek(), Some(3));
    assert!(q.offer(1).unwrap());
    assert_eq!(snapshot.drain().collect::<Vec<_>>(), [1, 3, 0, 2]);
    q.clear();
    assert!(q.is_empty());
    assert_eq!(q.peek(), None);
    assert!(!q.contains(3));
    assert!(q.offer(3).unwrap());
    assert_eq!(q.pop(), Some(3));
    let mut empty = NodeOrderQueue::new(Vec::<usize>::new());
    assert!(empty.offer(0).is_err());
    assert_eq!(empty.pop(), None);
}
#[test]
fn queues_keep_their_order_snapshot_when_graph_dependencies_change() {
    let mut graph = DependencyTopology::new(0..4).unwrap();
    graph.update(&[], &[(0, 3), (1, 0), (2, 1)]).unwrap();
    let old_ranks = graph
        .keys()
        .iter()
        .map(|n| graph.rank(n).unwrap())
        .collect::<Vec<_>>();
    let mut old = NodeOrderQueue::new(old_ranks.clone());
    for node in 0..4 {
        old.offer(node).unwrap();
    }
    graph
        .update(&[(0, 3), (1, 0), (2, 1)], &[(1, 2), (0, 1), (3, 0)])
        .unwrap();
    let mut current = NodeOrderQueue::new(
        graph
            .keys()
            .iter()
            .map(|n| graph.rank(n).unwrap())
            .collect::<Vec<_>>(),
    );
    for node in 0..4 {
        current.offer(node).unwrap();
    }
    assert_eq!(old.ranks(), old_ranks);
    assert_eq!(old.drain().collect::<Vec<_>>(), [3, 0, 1, 2]);
    assert_eq!(current.drain().collect::<Vec<_>>(), [2, 1, 0, 3]);
}
#[test]
fn cache_schedules_converging_dependencies_once_and_reorders_after_edge_edits() {
    let mut graph = DependencyTopology::new(0..5).unwrap();
    graph
        .update(&[], &[(1, 0), (2, 0), (3, 1), (3, 2)])
        .unwrap();
    let mut cache = DependencyCache::new(graph, |_, inputs| {
        Some(1 + inputs.iter().map(|(_, v)| v.value.unwrap()).sum::<i64>())
    });
    let old = cache.clone();
    let facts = [10, 1, 1, 1, 1];
    let mut calls = vec![];
    let changed = cache
        .update(&[], &[], &[3, 2, 0, 1, 0], |node, inputs| {
            calls.push(*node);
            Some(facts[*node] + inputs.iter().map(|(_, v)| v.value.unwrap()).sum::<i64>())
        })
        .unwrap();
    assert_eq!(changed, [0, 1, 2, 3]);
    assert_eq!(calls, [0, 1, 2, 3]);
    assert_eq!(cache.values()[&3].value, Some(23));
    assert_eq!(old.values()[&3].value, Some(5));
    calls.clear();
    cache
        .update(&[(1, 0)], &[(0, 1)], &[], |node, inputs| {
            calls.push(*node);
            Some(facts[*node] + inputs.iter().map(|(_, v)| v.value.unwrap()).sum::<i64>())
        })
        .unwrap();
    assert_eq!(calls, [1, 0, 2, 3]);
    assert_eq!(cache.values()[&3].value, Some(14));
    assert_eq!(cache.values()[&4].value, Some(1));
    cache
        .update(&[], &[(1, 3)], &[], |_, _| {
            panic!("cycle members must not invoke suppliers")
        })
        .unwrap();
    assert!((0..4).all(|node| cache.values()[&node].inconsistent));
    assert!(!cache.values()[&4].inconsistent);
    calls.clear();
    cache
        .update(&[(1, 3)], &[], &[], |node, inputs| {
            calls.push(*node);
            Some(facts[*node] + inputs.iter().map(|(_, v)| v.value.unwrap()).sum::<i64>())
        })
        .unwrap();
    assert_eq!(
        calls.iter().copied().collect::<BTreeSet<_>>(),
        [0, 1, 2, 3].into_iter().collect()
    );
    assert_eq!(calls.len(), 4);
    assert_eq!(cache.values()[&3].value, Some(14));
}
