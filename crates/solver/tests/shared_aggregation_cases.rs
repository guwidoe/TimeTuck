// Adapted from pinned Community Uni/Bi/Tri/Quad node-sharing groupBy tests.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
trait Facts: Clone + 'static {
    const ARITY: usize;
    fn first(&self) -> usize;
    fn second(&self) -> usize;
}
impl Facts for usize {
    const ARITY: usize = 1;
    fn first(&self) -> usize {
        *self
    }
    fn second(&self) -> usize {
        *self
    }
}
impl Facts for (usize, usize) {
    const ARITY: usize = 2;
    fn first(&self) -> usize {
        self.0
    }
    fn second(&self) -> usize {
        self.1
    }
}
impl Facts for ((usize, usize), usize) {
    const ARITY: usize = 3;
    fn first(&self) -> usize {
        self.0.0
    }
    fn second(&self) -> usize {
        self.0.1
    }
}
impl Facts for (((usize, usize), usize), usize) {
    const ARITY: usize = 4;
    fn first(&self) -> usize {
        self.0.0.0
    }
    fn second(&self) -> usize {
        self.0.0.1
    }
}
fn check<I: Ord + Clone + 'static, T: Facts>(
    b: &mut StreamGraphBuilder<'_, SimpleScore>,
    parent: &StreamPort<I, T>,
    case: usize,
) {
    match case {
        0 => {
            let d = SharedGroupKeys::new(|v: &T| v.first());
            let a = d.apply(b, parent).unwrap();
            let filtered = b.filter(parent, |_| true).unwrap();
            let c = d.apply(b, &filtered).unwrap();
            assert!(!a.same_node(&c));
        }
        1 => {
            let a = SharedGroupKeys::new(|v: &T| if T::ARITY == 1 { 0 } else { v.first() })
                .apply(b, parent)
                .unwrap();
            let c = SharedGroupKeys::new(|v: &T| if T::ARITY == 1 { 1 } else { v.second() })
                .apply(b, parent)
                .unwrap();
            assert!(!a.same_node(&c));
        }
        2 => {
            let distinct =
                SharedCollect::<I, T, usize, DistinctCountCollector<usize>>::new(|v| v.first())
                    .apply(b, parent)
                    .unwrap();
            if T::ARITY == 1 {
                let count = SharedCollect::<I, T, usize, CountCollector>::new(|v| v.first())
                    .apply(b, parent)
                    .unwrap();
                assert!(!count.same_node(&distinct));
            } else {
                let list = SharedCollectList::new(|v: &T| v.first())
                    .apply(b, parent)
                    .unwrap();
                assert!(!list.same_node(&distinct));
            }
        }
        3 => {
            let a = SharedCollect::<I, T, i64, SumCollector>::new(|_| 0)
                .apply(b, parent)
                .unwrap();
            // Quad deliberately uses two distinct functions which both return zero.
            let c =
                SharedCollect::<I, T, i64, SumCollector>::new(
                    |_| if T::ARITY == 4 { 0 } else { 1 },
                )
                .apply(b, parent)
                .unwrap();
            assert!(!a.same_node(&c));
        }
        4 => {
            let d = SharedGroupKeys::new(|v: &T| if T::ARITY <= 2 { 0 } else { v.first() });
            let a = d.apply(b, parent).unwrap();
            let c = d.apply(b, parent).unwrap();
            assert!(a.same_node(&c));
        }
        5 => {
            let d = SharedCollect::<I, T, i64, SumCollector>::new(|_| 0);
            let a = d.apply(b, parent).unwrap();
            let c = d.apply(b, parent).unwrap();
            assert!(a.same_node(&c));
        }
        _ => unreachable!(),
    }
}
fn run(arity: usize, case: usize) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let entities = b.source::<usize, usize>();
    if arity == 1 {
        return check(&mut b, entities.port(), case);
    }
    let pair = b
        .join_equal(entities.port(), entities.port(), |_| (), |_| ())
        .unwrap();
    if arity == 2 {
        return check(&mut b, &pair, case);
    }
    let tri = b
        .join_equal(&pair, entities.port(), |_| (), |_| ())
        .unwrap();
    if arity == 3 {
        return check(&mut b, &tri, case);
    }
    let quad = b.join_equal(&tri, entities.port(), |_| (), |_| ()).unwrap();
    check(&mut b, &quad, case);
}

#[test]
fn uni_different_parent_group_by() {
    run(1, 0);
}

#[test]
fn uni_different_key_mapper_group_by() {
    run(1, 1);
}

#[test]
fn uni_same_parent_different_collector_group_by() {
    run(1, 2);
}

#[test]
fn uni_same_parent_different_collector_function_group_by() {
    run(1, 3);
}

#[test]
fn uni_same_parent_same_key_mapper_group_by() {
    run(1, 4);
}

#[test]
fn uni_same_parent_same_collector_group_by() {
    run(1, 5);
}

#[test]
fn bi_different_parent_group_by() {
    run(2, 0);
}

#[test]
fn bi_different_key_mapper_group_by() {
    run(2, 1);
}

#[test]
fn bi_same_parent_different_collector_group_by() {
    run(2, 2);
}

#[test]
fn bi_same_parent_different_collector_function_group_by() {
    run(2, 3);
}

#[test]
fn bi_same_parent_same_key_mapper_group_by() {
    run(2, 4);
}

#[test]
fn bi_same_parent_same_collector_group_by() {
    run(2, 5);
}

#[test]
fn tri_different_parent_group_by() {
    run(3, 0);
}

#[test]
fn tri_different_key_mapper_group_by() {
    run(3, 1);
}

#[test]
fn tri_same_parent_different_collector_group_by() {
    run(3, 2);
}

#[test]
fn tri_same_parent_different_collector_function_group_by() {
    run(3, 3);
}

#[test]
fn tri_same_parent_same_key_mapper_group_by() {
    run(3, 4);
}

#[test]
fn tri_same_parent_same_collector_group_by() {
    run(3, 5);
}

#[test]
fn quad_different_parent_group_by() {
    run(4, 0);
}

#[test]
fn quad_different_key_mapper_group_by() {
    run(4, 1);
}

#[test]
fn quad_same_parent_different_collector_group_by() {
    run(4, 2);
}

#[test]
fn quad_same_parent_different_collector_function_group_by() {
    run(4, 3);
}

#[test]
fn quad_same_parent_same_key_mapper_group_by() {
    run(4, 4);
}

#[test]
fn quad_same_parent_same_collector_group_by() {
    run(4, 5);
}
