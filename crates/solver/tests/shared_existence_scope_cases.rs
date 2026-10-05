// Adapted from pinned Community Uni/Bi/Tri/Quad node-sharing tests (Apache-2.0).
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[derive(Clone)]
struct Fact {
    id: usize,
    code: String,
    assigned: Option<i32>,
}
trait Facts: Clone + 'static {
    const ARITY: usize;
    fn first(&self) -> &Fact;
    fn second(&self) -> &Fact;
}
impl Facts for Fact {
    const ARITY: usize = 1;
    fn first(&self) -> &Fact {
        self
    }
    fn second(&self) -> &Fact {
        self
    }
}
impl Facts for (Fact, Fact) {
    const ARITY: usize = 2;
    fn first(&self) -> &Fact {
        &self.0
    }
    fn second(&self) -> &Fact {
        &self.1
    }
}
impl Facts for ((Fact, Fact), Fact) {
    const ARITY: usize = 3;
    fn first(&self) -> &Fact {
        &self.0.0
    }
    fn second(&self) -> &Fact {
        &self.0.1
    }
}
impl Facts for (((Fact, Fact), Fact), Fact) {
    const ARITY: usize = 4;
    fn first(&self) -> &Fact {
        &self.0.0.0
    }
    fn second(&self) -> &Fact {
        &self.0.0.1
    }
}
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
enum Key {
    Any,
    Identity(usize),
    Code(String),
}
fn definition<I: Ord + Clone + 'static, T: Facts>(
    mode: ExistenceMode,
    other: bool,
    kind: usize,
    different: bool,
) -> SharedExistence<'static, I, usize, T, Fact, Key> {
    let indexed = kind == 1 || kind == 3;
    let d = SharedExistence::new(
        mode,
        move |a: &T| {
            if !indexed {
                Key::Any
            } else if different && T::ARITY == 1 {
                Key::Code(a.first().code.clone())
            } else {
                Key::Identity(if different {
                    a.second().id
                } else {
                    a.first().id
                })
            }
        },
        move |b: &Fact| {
            if !indexed {
                Key::Any
            } else if different && T::ARITY == 1 {
                Key::Code(b.code.clone())
            } else {
                Key::Identity(b.id)
            }
        },
    );
    if other || kind == 2 || kind == 4 {
        d.with_filter(move |a, b| {
            let equality = a.first().id == if T::ARITY == 1 { b.id } else { a.second().id };
            (!other || a.first().id != b.id) && (!(kind == 2 || kind == 4) || equality != different)
        })
    } else {
        d
    }
}
fn check<I: Ord + Clone + 'static, T: Facts>(
    b: &mut StreamGraphBuilder<'_, SimpleScore>,
    left: &StreamPort<I, T>,
    right: &StreamPort<usize, Fact>,
    mode: ExistenceMode,
    other: bool,
    kind: usize,
    quirk: bool,
) {
    let d = definition(mode, other, kind, false);
    let a = if quirk {
        definition(ExistenceMode::Exists, other, kind, false)
            .apply(b, left, right)
            .unwrap()
    } else {
        d.apply(b, left, right).unwrap()
    };
    let c = match kind {
        0 => {
            let changed = b.filter(left, |_| true).unwrap();
            d.apply(b, &changed, right).unwrap()
        }
        1 | 2 => d.apply(b, left, right).unwrap(),
        3 | 4 => definition(mode, other, kind, true)
            .apply(b, left, right)
            .unwrap(),
        _ => unreachable!(),
    };
    assert_eq!(a.same_node(&c), kind == 1 || kind == 2);
}
fn run(arity: usize, including: bool, other: bool, absent: bool, kind: usize) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let all = b.source::<usize, Fact>();
    let assigned = SharedFilter::new(|f: &Fact| f.assigned.is_some())
        .apply(&mut b, all.port())
        .unwrap();
    let right = if including { all.port() } else { &assigned };
    let mode = if absent {
        ExistenceMode::NotExists
    } else {
        ExistenceMode::Exists
    };
    // Exact source fixture: non-including unary ifNotExistsOtherDifferentParent
    // begins with ifExistsOther, then compares a filtered ifNotExistsOther.
    let quirk = arity == 1 && other && absent && !including && kind == 0;
    if arity == 1 {
        return check(&mut b, &assigned, right, mode, other, kind, quirk);
    }
    let bi = b.join_equal(&assigned, &assigned, |_| (), |_| ()).unwrap();
    if arity == 2 {
        return check(&mut b, &bi, right, mode, other, kind, false);
    }
    let tri = b.join_equal(&bi, &assigned, |_| (), |_| ()).unwrap();
    if arity == 3 {
        return check(&mut b, &tri, right, mode, other, kind, false);
    }
    let quad = b.join_equal(&tri, &assigned, |_| (), |_| ()).unwrap();
    check(&mut b, &quad, right, mode, other, kind, false);
}

#[test]
fn uni_if_exists_other_different_parent() {
    run(1, false, true, false, 0);
}

#[test]
fn uni_if_exists_other_same_parent_different_indexer() {
    run(1, false, true, false, 3);
}

#[test]
fn uni_if_exists_other_same_parent_different_filter() {
    run(1, false, true, false, 4);
}

#[test]
fn uni_if_exists_other_same_parent_same_indexer() {
    run(1, false, true, false, 1);
}

#[test]
fn uni_if_not_exists_other_different_parent() {
    run(1, false, true, true, 0);
}

#[test]
fn uni_if_not_exists_other_same_parent_different_indexer() {
    run(1, false, true, true, 3);
}

#[test]
fn uni_if_not_exists_other_same_parent_different_filter() {
    run(1, false, true, true, 4);
}

#[test]
fn uni_if_not_exists_other_same_parent_same_indexer() {
    run(1, false, true, true, 1);
}

#[test]
fn uni_if_exists_other_including_unassigned_different_parent() {
    run(1, true, true, false, 0);
}

#[test]
fn uni_if_exists_other_including_unassigned_same_parent_different_indexer() {
    run(1, true, true, false, 3);
}

#[test]
fn uni_if_exists_other_including_unassigned_same_parent_different_filter() {
    run(1, true, true, false, 4);
}

#[test]
fn uni_if_exists_other_including_unassigned_same_parent_same_indexer() {
    run(1, true, true, false, 1);
}

#[test]
fn uni_if_not_exists_other_including_unassigned_different_parent() {
    run(1, true, true, true, 0);
}

#[test]
fn uni_if_not_exists_other_including_unassigned_same_parent_different_indexer() {
    run(1, true, true, true, 3);
}

#[test]
fn uni_if_not_exists_other_including_unassigned_same_parent_different_filter() {
    run(1, true, true, true, 4);
}

#[test]
fn uni_if_not_exists_other_including_unassigned_same_parent_same_indexer() {
    run(1, true, true, true, 1);
}

#[test]
fn uni_if_exists_including_unassigned_different_parent() {
    run(1, true, false, false, 0);
}

#[test]
fn uni_if_not_exists_including_unassigned_different_parent() {
    run(1, true, false, true, 0);
}

#[test]
fn uni_if_exists_including_unassigned_same_parent_same_indexer() {
    run(1, true, false, false, 1);
}

#[test]
fn uni_if_exists_including_unassigned_same_parent_same_filter() {
    run(1, true, false, false, 2);
}

#[test]
fn uni_if_not_exists_including_unassigned_same_parent_same_indexer() {
    run(1, true, false, true, 1);
}

#[test]
fn uni_if_not_exists_including_unassigned_same_parent_same_filter() {
    run(1, true, false, true, 2);
}

#[test]
fn uni_if_exists_same_parent_different_indexer() {
    run(1, false, false, false, 3);
}

#[test]
fn uni_if_exists_same_parent_different_filter() {
    run(1, false, false, false, 4);
}

#[test]
fn uni_if_not_exists_same_parent_different_indexer() {
    run(1, false, false, true, 3);
}

#[test]
fn uni_if_not_exists_same_parent_different_filter() {
    run(1, false, false, true, 4);
}

#[test]
fn uni_if_exists_including_unassigned_same_parent_different_indexer() {
    run(1, true, false, false, 3);
}

#[test]
fn uni_if_exists_including_unassigned_same_parent_different_filter() {
    run(1, true, false, false, 4);
}

#[test]
fn uni_if_not_exists_including_unassigned_same_parent_different_indexer() {
    run(1, true, false, true, 3);
}

#[test]
fn uni_if_not_exists_including_unassigned_same_parent_different_filter() {
    run(1, true, false, true, 4);
}

#[test]
fn bi_if_exists_including_unassigned_different_parent() {
    run(2, true, false, false, 0);
}

#[test]
fn bi_if_not_exists_including_unassigned_different_parent() {
    run(2, true, false, true, 0);
}

#[test]
fn bi_if_exists_including_unassigned_same_parent_same_indexer() {
    run(2, true, false, false, 1);
}

#[test]
fn bi_if_exists_including_unassigned_same_parent_same_filter() {
    run(2, true, false, false, 2);
}

#[test]
fn bi_if_not_exists_including_unassigned_same_parent_same_indexer() {
    run(2, true, false, true, 1);
}

#[test]
fn bi_if_not_exists_including_unassigned_same_parent_same_filter() {
    run(2, true, false, true, 2);
}

#[test]
fn bi_if_exists_same_parent_different_indexer() {
    run(2, false, false, false, 3);
}

#[test]
fn bi_if_exists_same_parent_different_filter() {
    run(2, false, false, false, 4);
}

#[test]
fn bi_if_not_exists_same_parent_different_indexer() {
    run(2, false, false, true, 3);
}

#[test]
fn bi_if_not_exists_same_parent_different_filter() {
    run(2, false, false, true, 4);
}

#[test]
fn bi_if_exists_including_unassigned_same_parent_different_indexer() {
    run(2, true, false, false, 3);
}

#[test]
fn bi_if_exists_including_unassigned_same_parent_different_filter() {
    run(2, true, false, false, 4);
}

#[test]
fn bi_if_not_exists_including_unassigned_same_parent_different_indexer() {
    run(2, true, false, true, 3);
}

#[test]
fn bi_if_not_exists_including_unassigned_same_parent_different_filter() {
    run(2, true, false, true, 4);
}

#[test]
fn tri_if_exists_including_unassigned_different_parent() {
    run(3, true, false, false, 0);
}

#[test]
fn tri_if_not_exists_including_unassigned_different_parent() {
    run(3, true, false, true, 0);
}

#[test]
fn tri_if_exists_including_unassigned_same_parent_same_indexer() {
    run(3, true, false, false, 1);
}

#[test]
fn tri_if_exists_including_unassigned_same_parent_same_filter() {
    run(3, true, false, false, 2);
}

#[test]
fn tri_if_not_exists_including_unassigned_same_parent_same_indexer() {
    run(3, true, false, true, 1);
}

#[test]
fn tri_if_not_exists_including_unassigned_same_parent_same_filter() {
    run(3, true, false, true, 2);
}

#[test]
fn tri_if_exists_same_parent_different_indexer() {
    run(3, false, false, false, 3);
}

#[test]
fn tri_if_exists_same_parent_different_filter() {
    run(3, false, false, false, 4);
}

#[test]
fn tri_if_not_exists_same_parent_different_indexer() {
    run(3, false, false, true, 3);
}

#[test]
fn tri_if_not_exists_same_parent_different_filter() {
    run(3, false, false, true, 4);
}

#[test]
fn tri_if_exists_including_unassigned_same_parent_different_indexer() {
    run(3, true, false, false, 3);
}

#[test]
fn tri_if_exists_including_unassigned_same_parent_different_filter() {
    run(3, true, false, false, 4);
}

#[test]
fn tri_if_not_exists_including_unassigned_same_parent_different_indexer() {
    run(3, true, false, true, 3);
}

#[test]
fn tri_if_not_exists_including_unassigned_same_parent_different_filter() {
    run(3, true, false, true, 4);
}

#[test]
fn quad_if_exists_including_unassigned_different_parent() {
    run(4, true, false, false, 0);
}

#[test]
fn quad_if_not_exists_including_unassigned_different_parent() {
    run(4, true, false, true, 0);
}

#[test]
fn quad_if_exists_including_unassigned_same_parent_same_indexer() {
    run(4, true, false, false, 1);
}

#[test]
fn quad_if_exists_including_unassigned_same_parent_same_filter() {
    run(4, true, false, false, 2);
}

#[test]
fn quad_if_not_exists_including_unassigned_same_parent_same_indexer() {
    run(4, true, false, true, 1);
}

#[test]
fn quad_if_not_exists_including_unassigned_same_parent_same_filter() {
    run(4, true, false, true, 2);
}

#[test]
fn quad_if_exists_same_parent_different_indexer() {
    run(4, false, false, false, 3);
}

#[test]
fn quad_if_exists_same_parent_different_filter() {
    run(4, false, false, false, 4);
}

#[test]
fn quad_if_not_exists_same_parent_different_indexer() {
    run(4, false, false, true, 3);
}

#[test]
fn quad_if_not_exists_same_parent_different_filter() {
    run(4, false, false, true, 4);
}

#[test]
fn quad_if_exists_including_unassigned_same_parent_different_indexer() {
    run(4, true, false, false, 3);
}

#[test]
fn quad_if_exists_including_unassigned_same_parent_different_filter() {
    run(4, true, false, false, 4);
}

#[test]
fn quad_if_not_exists_including_unassigned_same_parent_different_indexer() {
    run(4, true, false, true, 3);
}

#[test]
fn quad_if_not_exists_including_unassigned_same_parent_different_filter() {
    run(4, true, false, true, 4);
}
