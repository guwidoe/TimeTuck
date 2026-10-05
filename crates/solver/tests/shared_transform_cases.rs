// Adapted from pinned Timefold Community stream node-sharing tests.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn same<I: Ord + Clone + 'static, T: Clone + PartialEq + 'static>(
    b: &mut StreamGraphBuilder<'_, SimpleScore>,
    a: &StreamPort<I, T>,
    c: &StreamPort<I, T>,
) -> bool {
    b.dataset(a).unwrap() == b.dataset(c).unwrap()
}
fn filter<I: Ord + Clone + 'static, T: Clone + PartialEq + 'static>(
    b: &mut StreamGraphBuilder<'_, SimpleScore>,
    parent: StreamPort<I, T>,
    case: usize,
) {
    let yes = SharedFilter::new(|_: &T| true);
    let no = SharedFilter::new(|_: &T| false);
    let first = yes.apply(b, &parent).unwrap();
    let other = match case {
        0 => {
            let middle = no.apply(b, &parent).unwrap();
            yes.apply(b, &middle).unwrap()
        }
        1 => no.apply(b, &parent).unwrap(),
        2 => yes.apply(b, &parent).unwrap(),
        _ => unreachable!(),
    };
    assert_eq!(same(b, &first, &other), case == 2);
}
fn filter_arity(arity: usize, case: usize) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i32>();
    if arity == 1 {
        return filter(&mut b, source.port().clone(), case);
    }
    let pair = b
        .join_equal(source.port(), source.port(), |_| (), |_| ())
        .unwrap();
    if arity == 2 {
        return filter(&mut b, pair, case);
    }
    let tri = b.join_equal(&pair, source.port(), |_| (), |_| ()).unwrap();
    if arity == 3 {
        return filter(&mut b, tri, case);
    }
    let quad = b.join_equal(&tri, source.port(), |_| (), |_| ()).unwrap();
    filter(&mut b, quad, case);
}
fn mapping(case: usize) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i32>();
    let identity = SharedMap::new(|v: &i32| Some(*v));
    let null = SharedMap::new(|_: &i32| None::<i32>);
    let a = identity.apply(&mut b, s.port()).unwrap();
    let other = match case {
        0 => {
            let p = b.filter(s.port(), |_| true).unwrap();
            identity.apply(&mut b, &p).unwrap()
        }
        1 => null.apply(&mut b, s.port()).unwrap(),
        2 => identity.apply(&mut b, s.port()).unwrap(),
        _ => unreachable!(),
    };
    assert_eq!(same(&mut b, &a, &other), case == 2);
}
fn flatten_last(case: usize) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i32>();
    let list = SharedFlatMap::new(|_: &i32| Vec::<i32>::new());
    let set = SharedFlatMap::new(|_: &i32| std::collections::BTreeSet::<i32>::new());
    let a = list.apply(&mut b, s.port()).unwrap();
    let other = match case {
        0 => {
            let p = b.filter(s.port(), |_| true).unwrap();
            list.apply(&mut b, &p).unwrap()
        }
        1 => set.apply(&mut b, s.port()).unwrap(),
        2 => list.apply(&mut b, s.port()).unwrap(),
        _ => unreachable!(),
    };
    assert_eq!(same(&mut b, &a, &other), case == 2);
}

#[test]
fn uni_different_parent_same_filter() {
    filter_arity(1, 0);
}

#[test]
fn uni_same_parent_different_filter() {
    filter_arity(1, 1);
}

#[test]
fn uni_same_parent_same_filter() {
    filter_arity(1, 2);
}

#[test]
fn bi_different_parent_same_filter() {
    filter_arity(2, 0);
}

#[test]
fn bi_same_parent_different_filter() {
    filter_arity(2, 1);
}

#[test]
fn bi_same_parent_same_filter() {
    filter_arity(2, 2);
}

#[test]
fn tri_different_parent_same_filter() {
    filter_arity(3, 0);
}

#[test]
fn tri_same_parent_different_filter() {
    filter_arity(3, 1);
}

#[test]
fn tri_same_parent_same_filter() {
    filter_arity(3, 2);
}

#[test]
fn quad_different_parent_same_filter() {
    filter_arity(4, 0);
}

#[test]
fn quad_same_parent_different_filter() {
    filter_arity(4, 1);
}

#[test]
fn quad_same_parent_same_filter() {
    filter_arity(4, 2);
}

#[test]
fn uni_different_parent_same_function_map() {
    mapping(0);
}

#[test]
fn uni_same_parent_different_function_map() {
    mapping(1);
}

#[test]
fn uni_same_parent_same_function_map() {
    mapping(2);
}

#[test]
fn uni_different_parent_same_function_flatten_last() {
    flatten_last(0);
}

#[test]
fn uni_same_parent_different_function_flatten_last() {
    flatten_last(1);
}

#[test]
fn uni_same_parent_same_function_flatten_last() {
    flatten_last(2);
}
