// Adapted from pinned Timefold Community *NodeComparatorTest and their
// AbstractNodeComparatorTest helper. Apache-2.0; see compat/provenance.json.
use timetuck::{NodeExploration as Order, SearchNodeKey, SimpleScore};
fn node(
    depth: usize,
    score: i64,
    unassigned: i64,
    bound: i64,
    parent: u64,
    breadth: u64,
) -> SearchNodeKey<SimpleScore> {
    SearchNodeKey {
        depth,
        score: SimpleScore::new(score).with_structural(-unassigned),
        optimistic_bound: Some(SimpleScore::new(bound)),
        parent_breadth: parent,
        breadth,
    }
}
fn n(depth: usize, score: i64, parent: u64, breadth: u64) -> SearchNodeKey<SimpleScore> {
    node(depth, score, 0, score, parent, breadth)
}
fn ordered(order: Order, nodes: Vec<SearchNodeKey<SimpleScore>>) {
    for (i, a) in nodes.iter().enumerate() {
        for b in &nodes[i + 1..] {
            assert!(order.compare(a, b).is_lt(), "{order:?}: {a:?} < {b:?}");
            assert!(order.compare(b, a).is_gt(), "{order:?}: {b:?} > {a:?}");
        }
    }
}
#[test]
fn depth_first_compare() {
    ordered(
        Order::DepthFirst,
        vec![
            n(1, -110, 5, 41),
            n(1, -110, 5, 40),
            n(1, -110, 7, 40),
            n(1, -90, 5, 40),
            n(1, -90, 7, 40),
            n(2, -110, 5, 40),
            n(2, -110, 7, 40),
            n(2, -90, 5, 40),
            n(2, -90, 7, 40),
            node(2, -80, 1, -80, 7, 40),
        ],
    );
}
#[test]
fn breadth_first_compare() {
    ordered(
        Order::BreadthFirst,
        vec![
            n(2, -110, 5, 51),
            n(2, -110, 5, 50),
            n(2, -90, 7, 41),
            n(2, -90, 5, 40),
            n(1, -110, 7, 61),
            n(1, -110, 5, 60),
            n(1, -90, 7, 71),
            n(1, -90, 5, 70),
            n(1, -85, 5, 60),
            node(1, -80, 1, -80, 5, 60),
        ],
    );
}
#[test]
fn score_first_compare() {
    ordered(
        Order::ScoreFirst,
        vec![
            n(1, -110, 5, 41),
            n(1, -110, 5, 40),
            n(1, -110, 7, 40),
            n(2, -110, 5, 40),
            n(2, -110, 7, 40),
            node(1, -95, 0, 0, 5, 40),
            node(2, -95, 0, 0, 5, 40),
            node(2, -95, 0, 0, 7, 40),
            n(1, -90, 5, 40),
            n(1, -90, 7, 40),
            n(2, -90, 5, 40),
            n(2, -90, 7, 40),
            node(1, -85, 1, -85, 5, 40),
            node(1, -80, 100, -80, 7, 40),
        ],
    );
}
#[test]
fn optimistic_bound_first_compare() {
    ordered(
        Order::OptimisticBoundFirst,
        vec![
            n(1, -300, 5, 41),
            n(1, -300, 5, 40),
            node(1, -200, 10, -200, 5, 40),
            n(1, -110, 5, 40),
            n(1, -110, 7, 40),
            n(2, -110, 5, 40),
            n(2, -110, 7, 40),
            n(1, -90, 5, 40),
            n(1, -90, 7, 40),
            n(2, -90, 5, 40),
            n(2, -90, 7, 40),
            node(1, -95, 0, 0, 5, 40),
            node(2, -95, 0, 0, 5, 40),
            node(2, -95, 0, 0, 7, 40),
            node(1, -11, 0, 1, 5, 40),
            node(1, -10, 1, 1, 5, 40),
        ],
    );
}
