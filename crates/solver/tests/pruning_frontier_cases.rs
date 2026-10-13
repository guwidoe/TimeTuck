// Adapted from pinned Community ExhaustiveSearchPhaseScopeTest and
// AbstractNodeComparatorTest.buildNode; Apache-2.0, see compat/provenance.json.
use timetuck::*;
#[test]
fn node_pruning() {
    let mut frontier = PruningFrontier::new(NodeExploration::ScoreFirst);
    for score in 0..3 {
        let score = SimpleScore::new(score);
        frontier.push(
            SearchNodeKey {
                depth: 0,
                score,
                optimistic_bound: Some(score),
                parent_breadth: 0,
                breadth: 0,
            },
            (),
        );
    }
    frontier
        .register_bound(SimpleScore::new(i64::from(i32::MIN)))
        .unwrap();
    frontier.register_bound(SimpleScore::new(1)).unwrap();
    assert_eq!(frontier.len(), 1);
}
