// Adapted from Community NodeTopologicalOrderQueueTest; Apache-2.0.
// See compat/provenance.json. Immutable rank snapshots replace graph mocks.
use std::collections::BTreeSet;
use timetuck::*;
fn queue(ranks: &[usize]) -> NodeOrderQueue {
    NodeOrderQueue::new(ranks.to_vec())
}
fn drain(q: &mut NodeOrderQueue) -> Vec<usize> {
    q.drain().collect()
}
#[test]
fn empty_queue() {
    let mut q = queue(&[0, 1, 2]);
    assert!(q.is_empty());
    assert_eq!(q.pop(), None);
}
#[test]
fn polls_in_topological_order_regardless_of_insertion_order() {
    let mut q = queue(&[30, 10, 20, 0]);
    for node in 0..4 {
        q.offer(node).unwrap();
    }
    assert_eq!(drain(&mut q), [3, 1, 2, 0]);
}
#[test]
fn drains_only_the_nodes_that_were_offered() {
    let mut q = queue(&[0, 10, 20, 30, 40]);
    q.offer(1).unwrap();
    q.offer(3).unwrap();
    assert_eq!(drain(&mut q), [1, 3]);
}
#[test]
fn offering_a_node_already_in_the_queue_does_nothing() {
    let mut q = queue(&[5, 1, 9]);
    for node in [2, 0, 2, 1, 0] {
        q.offer(node).unwrap();
    }
    assert_eq!(drain(&mut q), [1, 0, 2]);
}
#[test]
fn a_node_can_be_offered_again_once_it_has_been_polled() {
    let mut q = queue(&[0, 1]);
    q.offer(0).unwrap();
    assert_eq!(q.pop(), Some(0));
    q.offer(0).unwrap();
    q.offer(1).unwrap();
    assert_eq!(drain(&mut q), [0, 1]);
}
#[test]
fn holds_every_node_at_once() {
    let mut q = NodeOrderQueue::new((0..64).map(|n| 64 - n).collect::<Vec<_>>());
    for _ in 0..3 {
        for node in 0..64 {
            q.offer(node).unwrap();
        }
    }
    assert_eq!(drain(&mut q).len(), 64);
}
#[test]
fn nodes_sharing_a_topological_order_come_out_together() {
    let mut q = queue(&[7, 7, 3, 7]);
    for node in 0..4 {
        q.offer(node).unwrap();
    }
    let result = drain(&mut q);
    assert_eq!(result[0], 2);
    let mut rest = result[1..].to_vec();
    rest.sort_unstable();
    assert_eq!(rest, [0, 1, 3]);
}
#[test]
fn matches_a_priority_queue_on_random_workloads() {
    // Native deterministic PRNG; workload sizes/tie density match the upstream case.
    let mut seed = 0u64;
    let mut draw = |n: u64| {
        seed = seed.wrapping_add(0x9e3779b97f4a7c15);
        let mut x = seed;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
        ((x ^ (x >> 31)) % n) as usize
    };
    for attempt in 0..50 {
        let ranks = (0..200).map(|_| draw(50)).collect::<Vec<_>>();
        let mut q = queue(&ranks);
        let mut reference = BTreeSet::new();
        let mut present = [false; 200];
        let (mut actual, mut expected) = (vec![], vec![]);
        for _ in 0..1000 {
            if draw(2) != 0 {
                let node = draw(200);
                q.offer(node).unwrap();
                if !present[node] {
                    reference.insert((ranks[node], node));
                    present[node] = true;
                }
            } else if !q.is_empty() {
                actual.push(ranks[q.pop().unwrap()]);
                let (rank, node) = reference.pop_first().unwrap();
                present[node] = false;
                expected.push(rank);
            }
        }
        while let Some(node) = q.pop() {
            actual.push(ranks[node]);
            expected.push(reference.pop_first().unwrap().0);
        }
        assert_eq!(actual, expected, "attempt {attempt}");
    }
}
