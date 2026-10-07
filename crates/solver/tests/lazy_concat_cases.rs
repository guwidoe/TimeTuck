// Adapted from pinned ComposingListTest. Apache-2.0; see compat/provenance.json.
use std::cell::Cell;
use timetuck::*;
fn all<T: Clone>(c: &LazyConcat<'_, T>) -> Vec<T> {
    c.iter().map(|v| v.unwrap().clone()).collect()
}
#[test]
fn empty_sublists() {
    let mut c = LazyConcat::<String>::new();
    for _ in 0..3 {
        c.push(vec![]).unwrap();
    }
    assert!(c.is_empty());
}
#[test]
fn single_sublist() {
    let mut c = LazyConcat::new();
    c.push(vec!["a", "b", "c"]).unwrap();
    assert_eq!(c.len(), 3);
    assert_eq!(all(&c), ["a", "b", "c"]);
}
#[test]
fn multiple_sublists() {
    let mut c = LazyConcat::new();
    for v in [vec!["a", "b"], vec!["c", "d", "e"], vec!["f"]] {
        c.push(v).unwrap();
    }
    assert_eq!(c.len(), 6);
    assert_eq!(all(&c), ["a", "b", "c", "d", "e", "f"]);
}
#[test]
fn mixed_empty_and_non_empty_sublists() {
    let mut c = LazyConcat::new();
    for v in [vec![1, 2], vec![], vec![3, 4, 5], vec![], vec![6]] {
        c.push(v).unwrap();
    }
    assert_eq!(c.len(), 6);
    assert_eq!(all(&c), [1, 2, 3, 4, 5, 6]);
}
#[test]
fn on_demand_sublist() {
    let mut c = LazyConcat::new();
    c.push_deferred(3, || vec!["a", "b", "c"]).unwrap();
    assert_eq!(c.len(), 3);
    assert_eq!(all(&c), ["a", "b", "c"]);
}
#[test]
fn multiple_on_demand_sublists() {
    let mut c = LazyConcat::new();
    c.push_deferred(2, || vec!["a", "b"]).unwrap();
    c.push_deferred(3, || vec!["c", "d", "e"]).unwrap();
    c.push_deferred(1, || vec!["f"]).unwrap();
    assert_eq!(c.len(), 6);
    assert_eq!(all(&c), ["a", "b", "c", "d", "e", "f"]);
}
#[test]
fn mixed_eager_and_on_demand_sublists() {
    let mut c = LazyConcat::new();
    c.push(vec![1, 2]).unwrap();
    c.push_deferred(3, || vec![3, 4, 5]).unwrap();
    c.push(vec![6]).unwrap();
    assert_eq!(c.len(), 6);
    assert_eq!(all(&c), [1, 2, 3, 4, 5, 6]);
}
#[test]
fn empty_on_demand_sublist() {
    let mut c = LazyConcat::<String>::new();
    c.push_deferred(0, Vec::new).unwrap();
    assert!(c.is_empty());
}
#[test]
fn on_demand_sublist_lazy_initialization() {
    let called = Cell::new(false);
    let mut c = LazyConcat::new();
    c.push_deferred(2, || {
        called.set(true);
        vec!["a", "b"]
    })
    .unwrap();
    assert!(!called.get());
    assert_eq!(c.len(), 2);
    assert!(!called.get());
    assert_eq!(c.get(0), Ok(&"a"));
    assert!(called.get());
}
#[test]
fn on_demand_sublist_caches_result() {
    let calls = Cell::new(0);
    let mut c = LazyConcat::new();
    c.push_deferred(2, || {
        calls.set(calls.get() + 1);
        vec!["a", "b"]
    })
    .unwrap();
    for index in [0, 1, 0] {
        c.get(index).unwrap();
    }
    assert_eq!(calls.get(), 1);
}
fn four() -> LazyConcat<'static, &'static str> {
    let mut c = LazyConcat::new();
    c.push(vec!["a", "b"]).unwrap();
    c.push(vec!["c", "d"]).unwrap();
    c
}
#[test]
fn get_from_first_sublist() {
    let c = four();
    assert_eq!(c.get(0), Ok(&"a"));
    assert_eq!(c.get(1), Ok(&"b"));
}
#[test]
fn get_from_middle_sublist() {
    let mut c = four();
    c.push(vec!["e", "f"]).unwrap();
    assert_eq!(c.get(2), Ok(&"c"));
    assert_eq!(c.get(3), Ok(&"d"));
}
#[test]
fn get_from_last_sublist() {
    let c = four();
    assert_eq!(c.get(2), Ok(&"c"));
    assert_eq!(c.get(3), Ok(&"d"));
}
fn bounds(index: usize) {
    let mut c = LazyConcat::new();
    c.push(vec!["a", "b", "c"]).unwrap();
    let err = c.get(index).unwrap_err();
    assert_eq!(err, LazyConcatError::Index { index, length: 3 });
    let text = err.to_string();
    assert!(text.contains(&format!("Index: {index}")));
    assert!(text.contains("Size: 3"));
}
#[test]
fn index_out_of_bounds_at_size() {
    bounds(3);
}
#[test]
fn index_out_of_bounds_larger_than_size() {
    bounds(10);
}
#[test]
fn iteration() {
    let mut c = LazyConcat::new();
    c.push(vec!["a", "b"]).unwrap();
    c.push(vec!["c", "d", "e"]).unwrap();
    assert_eq!(all(&c), ["a", "b", "c", "d", "e"]);
}
#[test]
fn stream() {
    let mut c = LazyConcat::new();
    c.push(vec![1, 2]).unwrap();
    c.push(vec![3, 4, 5]).unwrap();
    assert_eq!(c.iter().map(|v| *v.unwrap()).sum::<i32>(), 15);
}
