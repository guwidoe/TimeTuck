// Adapted from pinned Community stream node-sharing tests (Apache-2.0).
// See compat/provenance.json and THIRD_PARTY_NOTICES.md for identities/substitutions.
use std::collections::BTreeSet;
use timetuck::*;
trait Facts: Ord + Clone + 'static {
    const ARITY: usize;
    fn fields(&self) -> Vec<Option<usize>>;
    fn first(&self) -> Option<usize> {
        self.fields()[0]
    }
    fn second(&self) -> Option<usize> {
        self.fields().get(1).copied().flatten()
    }
}
impl Facts for usize {
    const ARITY: usize = 1;
    fn fields(&self) -> Vec<Option<usize>> {
        vec![Some(*self)]
    }
}
impl Facts for (usize, usize) {
    const ARITY: usize = 2;
    fn fields(&self) -> Vec<Option<usize>> {
        vec![Some(self.0), Some(self.1)]
    }
}
impl Facts for ((usize, usize), usize) {
    const ARITY: usize = 3;
    fn fields(&self) -> Vec<Option<usize>> {
        vec![Some(self.0.0), Some(self.0.1), Some(self.1)]
    }
}
impl Facts for (((usize, usize), usize), usize) {
    const ARITY: usize = 4;
    fn fields(&self) -> Vec<Option<usize>> {
        vec![
            Some(self.0.0.0),
            Some(self.0.0.1),
            Some(self.0.1),
            Some(self.1),
        ]
    }
}
fn expanded<T: Facts>(v: &T, value: Option<usize>) -> Vec<Option<usize>> {
    let mut row = v.fields();
    row.push(value);
    row
}
fn flattened_last<T: Facts>(
    v: &T,
    values: impl IntoIterator<Item = usize>,
) -> Vec<Vec<Option<usize>>> {
    let mut prefix = v.fields();
    prefix.pop();
    values
        .into_iter()
        .map(|value| {
            let mut row = prefix.clone();
            row.push(Some(value));
            row
        })
        .collect()
}
fn check<I: Ord + Clone + 'static, T: Facts>(
    b: &mut StreamGraphBuilder<'_, SimpleScore>,
    p: &StreamPort<I, T>,
    kind: usize,
    case: usize,
) {
    let changed = b.filter(p, |_| true).unwrap();
    match kind {
        0 => {
            let d = SharedMap::new(|v: &T| expanded(v, v.first()));
            let a = d.apply(b, p).unwrap();
            let c = if case == 1 {
                SharedMap::new(|v: &T| expanded(v, v.second()))
                    .apply(b, p)
                    .unwrap()
            } else {
                d.apply(b, if case == 0 { &changed } else { p }).unwrap()
            };
            assert_eq!(a.same_node(&c), case == 2);
        }
        1 => {
            let d = SharedMap::new(|v: &T| v.first());
            let a = d.apply(b, p).unwrap();
            // Tri's second mapper also selects the first fact, but is a different callback.
            let c = if case == 1 {
                SharedMap::new(|v: &T| if T::ARITY == 3 { v.first() } else { v.second() })
                    .apply(b, p)
                    .unwrap()
            } else {
                d.apply(b, if case == 0 { &changed } else { p }).unwrap()
            };
            assert_eq!(a.same_node(&c), case == 2);
        }
        2 => {
            let d = SharedFlatMap::new(|_: &T| Vec::<usize>::new());
            let a = d.apply(b, p).unwrap();
            let c = if case == 1 {
                SharedFlatMap::new(|_: &T| BTreeSet::<usize>::new())
                    .apply(b, p)
                    .unwrap()
            } else {
                d.apply(b, if case == 0 { &changed } else { p }).unwrap()
            };
            assert_eq!(a.same_node(&c), case == 2);
        }
        3 => {
            let d = SharedFlatMap::new(|v: &T| flattened_last(v, Vec::<usize>::new()));
            let a = d.apply(b, p).unwrap();
            let c = if case == 1 {
                SharedFlatMap::new(|v: &T| flattened_last(v, BTreeSet::<usize>::new()))
                    .apply(b, p)
                    .unwrap()
            } else {
                d.apply(b, if case == 0 { &changed } else { p }).unwrap()
            };
            assert_eq!(a.same_node(&c), case == 2);
        }
        4 => {
            let d = SharedDistinct::new();
            let a = d.apply(b, p).unwrap();
            let c = d.apply(b, if case == 0 { &changed } else { p }).unwrap();
            assert_eq!(a.same_node(&c), case == 1);
        }
        5 => {
            let source_filter = SharedFilter::new(|v: &T| match T::ARITY {
                1 => v.first().is_some(),
                2 => v.first() != v.second(),
                _ => true,
            });
            let filter = SharedFilter::new(move |_: &T| case == 2 || T::ARITY <= 2);
            let filtered = filter.apply(b, p).unwrap();
            let changed = source_filter.apply(b, p).unwrap();
            let d = SharedConcat::new();
            let (a, c) = if case == 1 {
                (
                    d.apply(b, &filtered, p).unwrap(),
                    d.apply(b, &filtered, &changed).unwrap(),
                )
            } else {
                let a = d.apply(b, p, &filtered).unwrap();
                let filtered_again = filter.apply(b, p).unwrap();
                (
                    a,
                    d.apply(b, if case == 0 { &changed } else { p }, &filtered_again)
                        .unwrap(),
                )
            };
            assert_eq!(a.same_node(&c), case == 2);
        }
        _ => unreachable!(),
    }
}
fn padding<I: Ord + Clone + 'static, J: Ord + Clone + 'static, T: Facts, U: Facts>(
    b: &mut StreamGraphBuilder<'_, SimpleScore>,
    left: &StreamPort<I, T>,
    right: &StreamPort<J, U>,
) {
    let a = SharedPaddedConcat::new(
        |v: &T| {
            if T::ARITY < U::ARITY {
                expanded(v, v.first())
            } else {
                v.fields()
            }
        },
        |v: &U| {
            if U::ARITY < T::ARITY {
                expanded(v, v.first())
            } else {
                v.fields()
            }
        },
    )
    .apply(b, left, right)
    .unwrap();
    let c = SharedPaddedConcat::new(
        |v: &T| {
            if T::ARITY < U::ARITY {
                expanded(v, None)
            } else {
                v.fields()
            }
        },
        |v: &U| {
            if U::ARITY < T::ARITY {
                expanded(v, None)
            } else {
                v.fields()
            }
        },
    )
    .apply(b, left, right)
    .unwrap();
    assert!(!a.same_node(&c));
}
fn run(arity: usize, kind: usize, case: usize) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, usize>();
    if arity == 1 && kind != 6 {
        return check(&mut b, s.port(), kind, case);
    }
    let bi = b.join_equal(s.port(), s.port(), |_| (), |_| ()).unwrap();
    if kind == 6 && arity == 1 {
        return padding(&mut b, s.port(), &bi);
    }
    if arity == 2 {
        return if kind == 6 {
            padding(&mut b, &bi, s.port())
        } else {
            check(&mut b, &bi, kind, case)
        };
    }
    let tri = b.join_equal(&bi, s.port(), |_| (), |_| ()).unwrap();
    if arity == 3 {
        return if kind == 6 {
            padding(&mut b, &tri, &bi)
        } else {
            check(&mut b, &tri, kind, case)
        };
    }
    let quad = b.join_equal(&tri, s.port(), |_| (), |_| ()).unwrap();
    if kind == 6 {
        padding(&mut b, &quad, &tri)
    } else {
        check(&mut b, &quad, kind, case)
    }
}

#[test]
fn uni_different_parent_same_function_expand() {
    run(1, 0, 0);
}

#[test]
fn uni_same_parent_different_function_expand() {
    run(1, 0, 1);
}

#[test]
fn uni_same_parent_same_function_expand() {
    run(1, 0, 2);
}

#[test]
fn uni_different_parent_distinct() {
    run(1, 4, 0);
}

#[test]
fn uni_same_parent_distinct() {
    run(1, 4, 1);
}

#[test]
fn uni_different_first_source_concat() {
    run(1, 5, 0);
}

#[test]
fn uni_different_second_source_concat() {
    run(1, 5, 1);
}

#[test]
fn uni_same_sources_concat() {
    run(1, 5, 2);
}

#[test]
fn uni_different_padding_function_concat() {
    run(1, 6, 0);
}

#[test]
fn bi_different_parent_same_function_expand() {
    run(2, 0, 0);
}

#[test]
fn bi_same_parent_different_function_expand() {
    run(2, 0, 1);
}

#[test]
fn bi_same_parent_same_function_expand() {
    run(2, 0, 2);
}

#[test]
fn bi_different_parent_same_function_map() {
    run(2, 1, 0);
}

#[test]
fn bi_same_parent_different_function_map() {
    run(2, 1, 1);
}

#[test]
fn bi_same_parent_same_function_map() {
    run(2, 1, 2);
}

#[test]
fn bi_different_parent_same_function_flatten() {
    run(2, 2, 0);
}

#[test]
fn bi_same_parent_different_function_flatten() {
    run(2, 2, 1);
}

#[test]
fn bi_same_parent_same_function_flatten() {
    run(2, 2, 2);
}

#[test]
fn bi_different_parent_same_function_flatten_last() {
    run(2, 3, 0);
}

#[test]
fn bi_same_parent_different_function_flatten_last() {
    run(2, 3, 1);
}

#[test]
fn bi_same_parent_same_function_flatten_last() {
    run(2, 3, 2);
}

#[test]
fn bi_different_parent_distinct() {
    run(2, 4, 0);
}

#[test]
fn bi_same_parent_distinct() {
    run(2, 4, 1);
}

#[test]
fn bi_different_first_source_concat() {
    run(2, 5, 0);
}

#[test]
fn bi_different_second_source_concat() {
    run(2, 5, 1);
}

#[test]
fn bi_same_sources_concat() {
    run(2, 5, 2);
}

#[test]
fn bi_different_padding_function_concat() {
    run(2, 6, 0);
}

#[test]
fn tri_different_parent_same_function_expand() {
    run(3, 0, 0);
}

#[test]
fn tri_same_parent_different_function_expand() {
    run(3, 0, 1);
}

#[test]
fn tri_same_parent_same_function_expand() {
    run(3, 0, 2);
}

#[test]
fn tri_different_parent_same_function_map() {
    run(3, 1, 0);
}

#[test]
fn tri_same_parent_different_function_map() {
    run(3, 1, 1);
}

#[test]
fn tri_same_parent_same_function_map() {
    run(3, 1, 2);
}

#[test]
fn tri_different_parent_same_function_flatten() {
    run(3, 2, 0);
}

#[test]
fn tri_same_parent_different_function_flatten() {
    run(3, 2, 1);
}

#[test]
fn tri_same_parent_same_function_flatten() {
    run(3, 2, 2);
}

#[test]
fn tri_different_parent_same_function_flatten_last() {
    run(3, 3, 0);
}

#[test]
fn tri_same_parent_different_function_flatten_last() {
    run(3, 3, 1);
}

#[test]
fn tri_same_parent_same_function_flatten_last() {
    run(3, 3, 2);
}

#[test]
fn tri_different_parent_distinct() {
    run(3, 4, 0);
}

#[test]
fn tri_same_parent_distinct() {
    run(3, 4, 1);
}

#[test]
fn tri_different_first_source_concat() {
    run(3, 5, 0);
}

#[test]
fn tri_different_second_source_concat() {
    run(3, 5, 1);
}

#[test]
fn tri_same_sources_concat() {
    run(3, 5, 2);
}

#[test]
fn tri_different_padding_function_concat() {
    run(3, 6, 0);
}

#[test]
fn quad_different_parent_same_function_map() {
    run(4, 1, 0);
}

#[test]
fn quad_same_parent_different_function_map() {
    run(4, 1, 1);
}

#[test]
fn quad_same_parent_same_function_map() {
    run(4, 1, 2);
}

#[test]
fn quad_different_parent_same_function_flatten_last() {
    run(4, 3, 0);
}

#[test]
fn quad_same_parent_different_function_flatten_last() {
    run(4, 3, 1);
}

#[test]
fn quad_same_parent_same_function_flatten_last() {
    run(4, 3, 2);
}

#[test]
fn quad_different_parent_distinct() {
    run(4, 4, 0);
}

#[test]
fn quad_same_parent_distinct() {
    run(4, 4, 1);
}

#[test]
fn quad_different_first_source_concat() {
    run(4, 5, 0);
}

#[test]
fn quad_different_second_source_concat() {
    run(4, 5, 1);
}

#[test]
fn quad_same_sources_concat() {
    run(4, 5, 2);
}

#[test]
fn quad_different_padding_function_concat() {
    run(4, 6, 0);
}
