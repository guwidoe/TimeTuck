// Adapted from pinned Timefold Community Uni/Bi/Tri/Quad node-sharing tests.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[derive(Clone, PartialEq)]
struct Fact {
    id: usize,
    code: String,
}
trait Facts {
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
fn same<I: Ord + Clone + 'static, T: Clone + PartialEq + 'static>(
    b: &mut StreamGraphBuilder<'_, SimpleScore>,
    a: &StreamPort<I, T>,
    c: &StreamPort<I, T>,
) -> bool {
    b.dataset(a).unwrap() == b.dataset(c).unwrap()
}
fn join<I: Ord + Clone + 'static, T: Facts + Clone + PartialEq + 'static>(
    b: &mut StreamGraphBuilder<'_, SimpleScore>,
    left: &StreamPort<I, T>,
    right: &StreamPort<usize, Fact>,
    case: usize,
) {
    let indexed = case == 2 || case == 5;
    let base = SharedJoin::new(
        move |a: &T| indexed.then(|| a.first().code.clone()),
        move |v: &Fact| indexed.then(|| v.code.clone()),
    );
    let base = if case == 3 || case == 6 {
        base.with_filter(move |_, _| case == 6)
    } else {
        base
    };
    let a = base.apply(b, left, right).unwrap();
    let other = match case {
        0 => {
            let p = b.filter(left, |_| true).unwrap();
            base.apply(b, &p, right).unwrap()
        }
        1 => {
            let p = b.filter(right, |_| true).unwrap();
            base.apply(b, left, &p).unwrap()
        }
        2 => {
            let different = SharedJoin::new(
                |a: &T| Some(a.second().code.clone()),
                |v: &Fact| Some(v.code.clone()),
            );
            different.apply(b, left, right).unwrap()
        }
        3 => {
            let different = SharedJoin::new(|_: &T| None::<String>, |_: &Fact| None::<String>)
                .with_filter(|_, _| true);
            different.apply(b, left, right).unwrap()
        }
        4..=6 => base.apply(b, left, right).unwrap(),
        _ => unreachable!(),
    };
    assert_eq!(same(b, &a, &other), case >= 4);
}
fn exists<I: Ord + Clone + 'static, T: Facts + Clone + PartialEq + 'static>(
    b: &mut StreamGraphBuilder<'_, SimpleScore>,
    left: &StreamPort<I, T>,
    right: &StreamPort<usize, Fact>,
    mode: ExistenceMode,
    case: usize,
) {
    let base = SharedExistence::new(
        mode,
        move |a: &T| if case == 1 { a.first().id } else { 0 },
        move |a: &Fact| if case == 1 { a.id } else { 0 },
    );
    let base = if case == 2 {
        base.with_filter(|a, b| a.first().id == if T::ARITY == 1 { b.id } else { a.second().id })
    } else {
        base
    };
    let a = base.apply(b, left, right).unwrap();
    let other = if case == 0 {
        let p = b.filter(left, |_| true).unwrap();
        base.apply(b, &p, right).unwrap()
    } else {
        base.apply(b, left, right).unwrap()
    };
    assert_eq!(same(b, &a, &other), case != 0);
}
fn run(arity: usize, case: usize, mode: Option<ExistenceMode>) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let entities = b.source::<usize, Fact>();
    let values = b.source::<usize, Fact>();
    macro_rules! check {
        ($p:expr) => {
            if let Some(mode) = mode {
                exists(&mut b, $p, entities.port(), mode, case)
            } else {
                join(&mut b, $p, values.port(), case)
            }
        };
    }
    if arity == 1 {
        return check!(entities.port());
    }
    let pair = b
        .join_equal(entities.port(), entities.port(), |_| (), |_| ())
        .unwrap();
    if arity == 2 {
        return check!(&pair);
    }
    let tri = b
        .join_equal(&pair, entities.port(), |_| (), |_| ())
        .unwrap();
    if arity == 3 {
        return check!(&tri);
    }
    let quad = b.join_equal(&tri, entities.port(), |_| (), |_| ()).unwrap();
    check!(&quad);
}

#[test]
fn uni_different_left_parent_join() {
    run(1, 0, None);
}

#[test]
fn uni_different_right_parent_join() {
    run(1, 1, None);
}

#[test]
fn uni_same_parents_different_indexer_join() {
    run(1, 2, None);
}

#[test]
fn uni_same_parents_different_filtering_join() {
    run(1, 3, None);
}

#[test]
fn uni_same_parents_join() {
    run(1, 4, None);
}

#[test]
fn uni_same_parents_same_indexer_join() {
    run(1, 5, None);
}

#[test]
fn uni_same_parents_same_filtering_join() {
    run(1, 6, None);
}

#[test]
fn bi_different_left_parent_join() {
    run(2, 0, None);
}

#[test]
fn bi_different_right_parent_join() {
    run(2, 1, None);
}

#[test]
fn bi_same_parents_different_indexer_join() {
    run(2, 2, None);
}

#[test]
fn bi_same_parents_different_filtering_join() {
    run(2, 3, None);
}

#[test]
fn bi_same_parents_join() {
    run(2, 4, None);
}

#[test]
fn bi_same_parents_same_indexer_join() {
    run(2, 5, None);
}

#[test]
fn bi_same_parents_same_filtering_join() {
    run(2, 6, None);
}

#[test]
fn tri_different_left_parent_join() {
    run(3, 0, None);
}

#[test]
fn tri_different_right_parent_join() {
    run(3, 1, None);
}

#[test]
fn tri_same_parents_different_indexer_join() {
    run(3, 2, None);
}

#[test]
fn tri_same_parents_different_filtering_join() {
    run(3, 3, None);
}

#[test]
fn tri_same_parents_join() {
    run(3, 4, None);
}

#[test]
fn tri_same_parents_same_indexer_join() {
    run(3, 5, None);
}

#[test]
fn tri_same_parents_same_filtering_join() {
    run(3, 6, None);
}

#[test]
fn uni_if_exists_different_parent() {
    run(1, 0, Some(ExistenceMode::Exists));
}

#[test]
fn uni_if_exists_same_parent_same_indexer() {
    run(1, 1, Some(ExistenceMode::Exists));
}

#[test]
fn uni_if_exists_same_parent_same_filter() {
    run(1, 2, Some(ExistenceMode::Exists));
}

#[test]
fn uni_if_not_exists_different_parent() {
    run(1, 0, Some(ExistenceMode::NotExists));
}

#[test]
fn uni_if_not_exists_same_parent_same_indexer() {
    run(1, 1, Some(ExistenceMode::NotExists));
}

#[test]
fn uni_if_not_exists_same_parent_same_filter() {
    run(1, 2, Some(ExistenceMode::NotExists));
}

#[test]
fn bi_if_exists_different_parent() {
    run(2, 0, Some(ExistenceMode::Exists));
}

#[test]
fn bi_if_exists_same_parent_same_indexer() {
    run(2, 1, Some(ExistenceMode::Exists));
}

#[test]
fn bi_if_exists_same_parent_same_filter() {
    run(2, 2, Some(ExistenceMode::Exists));
}

#[test]
fn bi_if_not_exists_different_parent() {
    run(2, 0, Some(ExistenceMode::NotExists));
}

#[test]
fn bi_if_not_exists_same_parent_same_indexer() {
    run(2, 1, Some(ExistenceMode::NotExists));
}

#[test]
fn bi_if_not_exists_same_parent_same_filter() {
    run(2, 2, Some(ExistenceMode::NotExists));
}

#[test]
fn tri_if_exists_different_parent() {
    run(3, 0, Some(ExistenceMode::Exists));
}

#[test]
fn tri_if_exists_same_parent_same_indexer() {
    run(3, 1, Some(ExistenceMode::Exists));
}

#[test]
fn tri_if_exists_same_parent_same_filter() {
    run(3, 2, Some(ExistenceMode::Exists));
}

#[test]
fn tri_if_not_exists_different_parent() {
    run(3, 0, Some(ExistenceMode::NotExists));
}

#[test]
fn tri_if_not_exists_same_parent_same_indexer() {
    run(3, 1, Some(ExistenceMode::NotExists));
}

#[test]
fn tri_if_not_exists_same_parent_same_filter() {
    run(3, 2, Some(ExistenceMode::NotExists));
}

#[test]
fn quad_if_exists_different_parent() {
    run(4, 0, Some(ExistenceMode::Exists));
}

#[test]
fn quad_if_exists_same_parent_same_indexer() {
    run(4, 1, Some(ExistenceMode::Exists));
}

#[test]
fn quad_if_exists_same_parent_same_filter() {
    run(4, 2, Some(ExistenceMode::Exists));
}

#[test]
fn quad_if_not_exists_different_parent() {
    run(4, 0, Some(ExistenceMode::NotExists));
}

#[test]
fn quad_if_not_exists_same_parent_same_indexer() {
    run(4, 1, Some(ExistenceMode::NotExists));
}

#[test]
fn quad_if_not_exists_same_parent_same_filter() {
    run(4, 2, Some(ExistenceMode::NotExists));
}
