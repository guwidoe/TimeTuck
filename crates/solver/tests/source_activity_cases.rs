// Adapted from pinned Community ForEachUniNodeActivityTest,
// IfExistsUniNodeActivityTest and ConcatUniUniNodeActivityTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn source_active(filtered: Option<bool>, fact: bool, downstream: bool) -> bool {
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let source = b.source::<usize, &'static str>();
    let out = match filtered {
        Some(pass) => b.filter(source.port(), move |_| pass).unwrap(),
        None => source.port().clone(),
    };
    b.score_weighted("sink", &out, SimpleScore::new(i64::from(downstream)), |_| 1)
        .unwrap();
    let mut g = b.build();
    if fact {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::put(0, "a")).unwrap();
        t.commit().unwrap();
    }
    g.activity().get(&out).unwrap().active
}
#[test]
fn unfiltered_inactive_when_no_facts() {
    assert!(!source_active(None, false, true));
}
#[test]
fn unfiltered_active_when_facts_exist() {
    assert!(source_active(None, true, true));
}
#[test]
fn unfiltered_inactive_when_downstream_inactive() {
    assert!(!source_active(None, true, false));
}
#[test]
fn filtered_inactive_when_no_facts() {
    assert!(!source_active(Some(true), false, true));
}
#[test]
fn filtered_active_when_fact_inserted_even_if_filtered_out() {
    assert!(source_active(Some(false), true, true));
}
#[test]
fn filtered_inactive_when_downstream_inactive() {
    assert!(!source_active(Some(true), true, false));
}
fn settle<I, T>(
    b: StreamGraphBuilder<'static, SimpleScore>,
    l: StreamSource<usize, &'static str>,
    r: StreamSource<usize, &'static str>,
    out: StreamPort<I, T>,
    left: bool,
    right: bool,
) -> bool {
    let mut g = b.build();
    let mut tx = g.transaction();
    if left {
        tx.apply(&l, TupleChange::put(0, "a")).unwrap();
    }
    if right {
        tx.apply(&r, TupleChange::put(0, "b")).unwrap();
    }
    tx.commit().unwrap();
    g.activity().get(&out).unwrap().active
}
fn exists_active(exists: bool, left: bool, right: bool, downstream: bool) -> bool {
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let l = b.source();
    let r = b.source();
    let out = b
        .existence_equal(
            l.port(),
            r.port(),
            if exists {
                ExistenceMode::Exists
            } else {
                ExistenceMode::NotExists
            },
            |_| (),
            |_| (),
        )
        .unwrap();
    b.score_weighted("sink", &out, SimpleScore::new(i64::from(downstream)), |_| 1)
        .unwrap();
    settle(b, l, r, out, left, right)
}
#[test]
fn if_exists_active_when_both_produce() {
    assert!(exists_active(true, true, true, true));
}
#[test]
fn if_exists_inactive_when_right_empty() {
    assert!(!exists_active(true, true, false, true));
}
#[test]
fn if_exists_inactive_when_left_empty() {
    assert!(!exists_active(true, false, true, true));
}
#[test]
fn if_not_exists_active_when_right_empty() {
    assert!(exists_active(false, true, false, true));
}
#[test]
fn if_not_exists_active_when_right_produces() {
    assert!(exists_active(false, true, true, true));
}
#[test]
fn if_not_exists_inactive_when_left_empty() {
    assert!(!exists_active(false, false, false, true));
}
#[test]
fn if_not_exists_inactive_when_downstream_inactive() {
    assert!(!exists_active(false, true, false, false));
}
fn concat_active(left: bool, right: bool, downstream: bool) -> bool {
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let l = b.source();
    let r = b.source();
    let out = b.concat(l.port(), r.port()).unwrap();
    b.score_weighted("sink", &out, SimpleScore::new(i64::from(downstream)), |_| 1)
        .unwrap();
    settle(b, l, r, out, left, right)
}
#[test]
fn concat_active_when_both_sides_produce() {
    assert!(concat_active(true, true, true));
}
#[test]
fn concat_active_when_only_left_produces() {
    assert!(concat_active(true, false, true));
}
#[test]
fn concat_active_when_only_right_produces() {
    assert!(concat_active(false, true, true));
}
#[test]
fn concat_inactive_when_neither_produces() {
    assert!(!concat_active(false, false, true));
}
#[test]
fn concat_inactive_when_downstream_inactive() {
    assert!(!concat_active(true, true, false));
}
// Additional complete methods from SingleInputNodeActivityTest and
// JoinBiNodeActivityTest. Methods asserting lifecycle notifications stay pending.
fn mapped_active(upstream: bool, downstream: bool) -> bool {
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let l = b.source::<usize, &'static str>();
    let r = b.source();
    let out = b.map(l.port(), |s| *s).unwrap();
    b.score_weighted("sink", &out, SimpleScore::new(i64::from(downstream)), |_| 1)
        .unwrap();
    settle(b, l, r, out, upstream, false)
}
#[test]
fn single_active_when_upstream_and_downstream_active() {
    assert!(mapped_active(true, true));
}
#[test]
fn single_inactive_when_upstream_cannot_produce() {
    assert!(!mapped_active(false, true));
}
#[test]
fn single_inactive_when_downstream_inactive() {
    assert!(!mapped_active(true, false));
}
#[test]
fn join_inactive_when_downstream_inactive() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let l = b.source();
    let r = b.source();
    let out = b.join_equal(l.port(), r.port(), |_| (), |_| ()).unwrap();
    b.score_weighted("sink", &out, SimpleScore::ZERO, |_| 1)
        .unwrap();
    assert!(!settle(b, l, r, out, true, true));
}
