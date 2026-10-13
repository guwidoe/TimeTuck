// Adapted from Community BavetNodeDeactivationTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;

#[test]
fn chain_over_empty_class_fully_deactivates() {
    let mut builder = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let extras = builder.source::<usize, i64>();
    // A true filter and identity map share one physical mapping node; no separate
    // filter node is registered. Native score terminals also count as nodes.
    let mapped = builder
        .filter_map(extras.port(), |extra| {
            let passes = true;
            passes.then_some(*extra)
        })
        .unwrap();
    builder
        .score("chainOverExtras", &mapped, |_| SimpleScore::new(-1))
        .unwrap();
    let graph = builder.build();
    let activity = graph.activity();
    assert_eq!(activity.active_count(), 0);
    assert!(!activity.get(extras.port()).unwrap().active);
    assert!(!activity.get(&mapped).unwrap().active);
    assert_eq!(activity.node_count(), 3); // Source, fused map, score; no filter.
}
