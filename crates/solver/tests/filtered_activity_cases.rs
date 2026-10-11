// Adapted from pinned Community ForEachFilteredUniNodeActivityTest.
// Apache-2.0; provenance additions accompany this batch.
use timetuck::*;
fn activity(
    changes: impl IntoIterator<Item = TupleChange<&'static str, bool>>,
) -> (NodeActivity, NodeActivity) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let facts = b.source::<&'static str, bool>();
    let filtered = b.filter(facts.port(), |passes| *passes).unwrap();
    let downstream = b.map(&filtered, |passes| *passes).unwrap();
    b.score("observed", &downstream, |_| SimpleScore::new(1))
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for change in changes {
        tx.apply(&facts, change).unwrap();
    }
    tx.commit().unwrap();
    let a = g.activity();
    (a.get(&filtered).unwrap(), a.get(&downstream).unwrap())
}
#[test]
fn stays_active_when_fact_stops_passing_filter_before_activation_check() {
    let (node, downstream) = activity([TupleChange::put("A", true), TupleChange::put("A", false)]);
    assert!(node.active);
    assert!(downstream.can_produce);
}
#[test]
fn deactivates_when_all_facts_retracted_after_filter_transition() {
    let (node, downstream) = activity([
        TupleChange::put("A", false),
        TupleChange::put("A", true),
        TupleChange::retract("A"),
    ]);
    assert!(!node.active);
    assert!(!downstream.can_produce);
}
#[test]
fn stays_active_when_fact_never_passed_filter() {
    let (node, downstream) = activity([TupleChange::put("A", false)]);
    assert!(node.active);
    assert!(downstream.can_produce);
}
#[test]
fn inactive_when_no_facts_inserted() {
    let (node, downstream) = activity([]);
    assert!(!node.active);
    assert!(!downstream.can_produce);
}
