// Adapted from pinned Community Abstract{Uni,Bi,Tri,Quad}ConstraintStreamNodeSharingTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::cell::Cell;
use timetuck::*;

fn compare<'a, I: Ord + Clone + 'static, T: Clone + PartialEq + 'static>(
    mut b: StreamGraphBuilder<'a, SimpleScore>,
    parent: StreamPort<I, T>,
    source: StreamSource<usize, i32>,
    same: bool,
    arity: u32,
    calls: &'a Cell<usize>,
) {
    let yes = SharedFilter::new(|_: &T| true);
    let no = SharedFilter::new(|_: &T| false);
    let cache = SharedPrecompute::new(
        |r: &T| r.clone(),
        |r: &T| {
            calls.set(calls.get() + 1);
            r.clone()
        },
    );
    let first_parent = yes.apply(&mut b, &parent).unwrap();
    let first = cache.apply(&mut b, &first_parent).unwrap();
    let second_parent = if same {
        yes.clone().apply(&mut b, &parent).unwrap()
    } else {
        no.apply(&mut b, &parent).unwrap()
    };
    let second = cache.clone().apply(&mut b, &second_parent).unwrap();
    assert_eq!(first.same_node(&second), same);
    // Extra native execution evidence: both consumers share the fact cache.
    b.score("first", &first, |_| SimpleScore::new(-1)).unwrap();
    b.score("second", &second, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for id in 0..2 {
        tx.apply(&source, TupleChange::put(id, id as i32)).unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(
        g.score(),
        SimpleScore::new(-(2_i64.pow(arity) * (if same { 2 } else { 1 })))
    );
    assert_eq!(calls.get(), 2_usize.pow(arity));
}
fn run(arity: u32, same: bool) {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i32>();
    // This root is the fact-only retrieval scope; ordinary source.port() stays live.
    let fact_scope = SharedPrecompute::new(|v: &i32| *v, |v: &i32| *v);
    let facts = fact_scope.apply(&mut b, source.port()).unwrap();
    if arity == 1 {
        return compare(b, facts, source, same, arity, &calls);
    }
    let join = SharedJoin::new(|_: &i32| (), |_: &i32| ());
    let pair = join.apply(&mut b, &facts, &facts).unwrap();
    if arity == 2 {
        return compare(b, pair, source, same, arity, &calls);
    }
    let join = SharedJoin::new(|_: &(i32, i32)| (), |_: &i32| ());
    let tri = join.apply(&mut b, &pair, &facts).unwrap();
    if arity == 3 {
        return compare(b, tri, source, same, arity, &calls);
    }
    let join = SharedJoin::new(|_: &((i32, i32), i32)| (), |_: &i32| ());
    let quad = join.apply(&mut b, &tri, &facts).unwrap();
    compare(b, quad, source, same, arity, &calls);
}
#[test]
fn uni_same_retrival_semantics_for_each() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i32>();
    assert!(source.port().same_node(source.port()));
}
#[test]
fn uni_different_retrival_semantics_for_each() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i32>();
    let scope = SharedPrecompute::new(|v: &i32| *v, |v: &i32| *v);
    let facts = scope.apply(&mut b, source.port()).unwrap();
    assert!(!source.port().same_node(&facts));
}

#[test]
fn uni_same_data_precompute() {
    run(1, true);
}

#[test]
fn uni_different_data_precompute() {
    run(1, false);
}

#[test]
fn bi_same_data_precompute() {
    run(2, true);
}

#[test]
fn bi_different_data_precompute() {
    run(2, false);
}

#[test]
fn tri_same_data_precompute() {
    run(3, true);
}

#[test]
fn tri_different_data_precompute() {
    run(3, false);
}

#[test]
fn quad_same_data_precompute() {
    run(4, true);
}

#[test]
fn quad_different_data_precompute() {
    run(4, false);
}
