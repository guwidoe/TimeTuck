// Adapted from pinned Community ForEachFilteredUniNodeActivityTest.
// Apache-2.0; provenance additions accompany this batch.
use std::cell::RefCell;
use timetuck::*;
fn activity(
    changes: impl IntoIterator<Item = TupleChange<&'static str, bool>>,
) -> (NodeActivity, NodeActivity) {
    let notifications = RefCell::new(Vec::new());
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let facts = b.source::<&'static str, bool>();
    let filtered = b.filter(facts.port(), |passes| *passes).unwrap();
    let downstream = b.map(&filtered, |passes| *passes).unwrap();
    b.score("observed", &downstream, |_| SimpleScore::new(1))
        .unwrap();
    b.after_initial_facts(&filtered, |can| notifications.borrow_mut().push(can))
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for change in changes {
        tx.apply(&facts, change).unwrap();
    }
    tx.commit().unwrap();
    let a = g.activity();
    assert_eq!(g.finish_initial_facts(), 1);
    assert_eq!(
        *notifications.borrow(),
        vec![a.get(&filtered).unwrap().can_produce]
    );
    assert_eq!(g.finish_initial_facts(), 0);
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
